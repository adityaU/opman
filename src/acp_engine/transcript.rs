//! In-memory opencode transcript, built from ACP session updates.
//!
//! The `claude -p` engine treated the CLI's on-disk JSONL as authoritative and re-parsed
//! the whole file after every content block. ACP removes the need: the update stream is
//! complete and ordered, so the transcript is folded once, incrementally, and never
//! re-read. That is what makes streaming cheap — a token costs one append and one small
//! event instead of a file parse.
//!
//! Part ids are `{message_id}:{index}`, matching what the shared transcript parser
//! produces, so anything that renders a claude session renders these unchanged.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::emit::{append_text, new_tool_part, part_id, Chunk, Emit};
use crate::claude_engine::jsonl::MsgOut;

/// The rendered conversation for one opman session.
///
/// Fields are `pub(super)` rather than private because the replay half of this type lives in
/// [`super::transcript_replay`]: history and live folding pull on the same state, and one
/// file holding both would be too long to read as either story.
#[derive(Default)]
pub struct Transcript {
    pub(super) session_id: String,
    pub(super) messages: Vec<MsgOut>,
    /// message id → index in `messages`.
    pub(super) index: HashMap<String, usize>,
    /// ACP `toolCallId` → (message index, part index).
    pub(super) tools: HashMap<String, (usize, usize)>,
    /// The assistant message currently receiving updates.
    pub(super) live: Option<String>,
    /// Index of the open streaming part within `live`, and its kind, so consecutive
    /// chunks of the same kind append instead of creating a part per token.
    pub(super) open: Option<(usize, Chunk)>,
    pub(super) user_turn: u64,
    pub(super) assistant_turn: u64,
    pub(super) model: String,
    /// Prompts set aside for the duration of a replay — see `begin_replay`.
    pub(super) held: Vec<MsgOut>,
    /// Whether the frames arriving are a `session/load` replay rather than a live turn.
    pub(super) replaying: bool,
}

impl Transcript {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            ..Default::default()
        }
    }

    pub fn messages(&self) -> &[MsgOut] {
        &self.messages
    }

    pub fn set_model(&mut self, model: &str) {
        if !model.is_empty() {
            self.model = model.to_string();
        }
    }

    /// Fold one streamed content chunk into the live assistant message.
    pub fn chunk(
        &mut self,
        kind: Chunk,
        message_id: Option<&str>,
        text: &str,
        out: &mut Vec<Emit>,
    ) {
        if text.is_empty() {
            return;
        }
        let mid = self.ensure_assistant(message_id, out);
        let Some(&idx) = self.index.get(&mid) else {
            return;
        };

        // Continue the open part when the kind matches; a tool call in between closes it,
        // because the text after a tool call is a new part in the rendered message.
        if let Some((part_idx, open_kind)) = self.open {
            if open_kind == kind {
                if let Some(part) = self.messages[idx].parts.get_mut(part_idx) {
                    let Some(delta) = append_text(part, text) else {
                        return;
                    };
                    out.push(Emit::Delta {
                        session_id: self.session_id.clone(),
                        message_id: mid,
                        part_id: part_id(part),
                        delta,
                    });
                    return;
                }
            }
        }

        let part_idx = self.messages[idx].parts.len();
        let part = json!({
            "type": kind.part_type(),
            "id": format!("{mid}:{part_idx}"),
            "messageID": mid,
            "sessionID": self.session_id,
            "text": text,
        });
        self.messages[idx].parts.push(part.clone());
        self.open = Some((part_idx, kind));
        out.push(Emit::Part(part));
    }

    /// A block from the agent that is not prose — an image, a sound, an embedded blob — as a
    /// `file` part on the live assistant message. The timeline already renders these for a
    /// user's attachments, so an agent's arrive with no frontend change.
    pub fn file(
        &mut self,
        message_id: Option<&str>,
        file: &super::content::Rendered,
        out: &mut Vec<Emit>,
    ) {
        let super::content::Rendered::File {
            mime,
            filename,
            url,
        } = file
        else {
            return;
        };
        let mid = self.ensure_assistant(message_id, out);
        let Some(&idx) = self.index.get(&mid) else {
            return;
        };
        let part_idx = self.messages[idx].parts.len();
        let part = json!({
            "type": "file",
            "id": format!("{mid}:{part_idx}"),
            "messageID": mid,
            "sessionID": self.session_id,
            "mime": mime,
            "filename": filename,
            "url": url,
        });
        self.messages[idx].parts.push(part.clone());
        // Prose after a file starts a new part, exactly as it does after a tool call.
        self.open = None;
        out.push(Emit::Part(part));
    }

    /// Create or merge a tool part. ACP sends `tool_call` then a run of `tool_call_update`
    /// frames that each carry only the fields that changed, so this merges rather than
    /// replaces — and an update for an unseen id creates the part, since a client that
    /// joined late must not drop the call.
    pub fn tool_upsert(&mut self, update: &Value, out: &mut Vec<Emit>) {
        let Some(call_id) = update.get("toolCallId").and_then(Value::as_str) else {
            return;
        };
        let located = self.tools.get(call_id).copied();
        let (idx, part_idx) = match located {
            Some(loc) => loc,
            None => {
                let mid = self.ensure_assistant(None, out);
                let Some(&idx) = self.index.get(&mid) else {
                    return;
                };
                let part_idx = self.messages[idx].parts.len();
                self.messages[idx]
                    .parts
                    .push(new_tool_part(&self.session_id, &mid, call_id));
                self.tools.insert(call_id.to_string(), (idx, part_idx));
                (idx, part_idx)
            }
        };
        // Text that follows a tool call belongs to a new part.
        self.open = None;

        let Some(part) = self.messages[idx].parts.get_mut(part_idx) else {
            return;
        };
        super::tool::merge(part, update);
        out.push(Emit::Part(part.clone()));
    }

    /// Refresh the live assistant envelope with token/cost figures.
    pub fn set_usage(&mut self, tokens: Value, cost: Option<f64>, out: &mut Vec<Emit>) {
        let Some(mid) = self.live.clone() else { return };
        let Some(&idx) = self.index.get(&mid) else {
            return;
        };
        let info = &mut self.messages[idx].info;
        info["tokens"] = tokens;
        if let Some(cost) = cost {
            info["cost"] = json!(cost);
        }
        out.push(Emit::Message(info.clone()));
    }

    /// End of turn: stamp completion on the live message and settle any tool left running
    /// (an aborted turn leaves calls that would otherwise spin forever in the UI).
    pub fn finish_turn(&mut self, out: &mut Vec<Emit>) {
        let Some(mid) = self.live.take() else { return };
        self.open = None;
        let Some(&idx) = self.index.get(&mid) else {
            return;
        };
        let ts = super::now_ms();
        for part in self.messages[idx].parts.iter_mut() {
            if super::tool::settle(part, ts) {
                out.push(Emit::Part(part.clone()));
            }
        }
        let info = &mut self.messages[idx].info;
        info["time"]["completed"] = json!(ts);
        out.push(Emit::Message(info.clone()));
    }

    /// The live assistant message, creating it if this is the first update of a turn.
    /// `message_id` is the agent's own id when it supplies one (Claude does), which keeps
    /// opman's ids aligned with the agent's across a reload.
    fn ensure_assistant(&mut self, message_id: Option<&str>, out: &mut Vec<Emit>) -> String {
        let mid = match message_id.filter(|m| !m.is_empty()) {
            Some(m) => m.to_string(),
            None => match &self.live {
                Some(live) => return live.clone(),
                None => {
                    self.assistant_turn += 1;
                    format!("msg_asst_{}_{}", self.session_id, self.assistant_turn)
                }
            },
        };
        if self.live.as_deref() == Some(mid.as_str()) {
            return mid;
        }
        // The agent moved to a new message; the previous one is done.
        if self.live.is_some() {
            self.finish_turn(out);
        }
        self.live = Some(mid.clone());
        self.open = None;
        if self.index.contains_key(&mid) {
            return mid;
        }
        let info = json!({
            "role": "assistant",
            "id": mid,
            "sessionID": self.session_id,
            "model": self.model,
            "cost": 0.0,
            "tokens": json!({}),
            "time": { "created": super::now_ms() },
        });
        self.index.insert(mid.clone(), self.messages.len());
        self.messages.push(MsgOut {
            info: info.clone(),
            parts: vec![],
        });
        out.push(Emit::Message(info));
        mid
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod transcript_tests;
