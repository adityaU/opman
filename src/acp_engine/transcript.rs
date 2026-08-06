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

use super::attach::Prompt;
use super::emit::{append_text, new_tool_part, part_id, Chunk, Emit};
use crate::claude_engine::jsonl::MsgOut;

/// The rendered conversation for one opman session.
#[derive(Default)]
pub struct Transcript {
    session_id: String,
    messages: Vec<MsgOut>,
    /// message id → index in `messages`.
    index: HashMap<String, usize>,
    /// ACP `toolCallId` → (message index, part index).
    tools: HashMap<String, (usize, usize)>,
    /// The assistant message currently receiving updates.
    live: Option<String>,
    /// Index of the open streaming part within `live`, and its kind, so consecutive
    /// chunks of the same kind append instead of creating a part per token.
    open: Option<(usize, Chunk)>,
    user_turn: u64,
    assistant_turn: u64,
    model: String,
    /// Prompts set aside for the duration of a replay — see [`Self::begin_replay`].
    held: Vec<MsgOut>,
    /// Whether the frames arriving are a `session/load` replay rather than a live turn.
    replaying: bool,
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

    /// Hand history back to the agent: `session/load` is about to replay the conversation,
    /// so what is rendered now would double.
    ///
    /// The trailing user messages are held rather than dropped. They are the prompt that
    /// triggered this connection — opman rendered it optimistically, the agent has not
    /// received it, and so the replay will not contain it. Clearing it outright is what
    /// made a first message after a restart vanish from the transcript.
    ///
    /// Turn counters keep counting: the ids they generate must not collide with the ones
    /// already handed to the held messages and broadcast to the client.
    pub fn begin_replay(&mut self) {
        let split = self
            .messages
            .iter()
            .rposition(|m| m.info["role"] != "user")
            .map_or(0, |last| last + 1);
        self.held = self.messages.split_off(split);
        self.messages.clear();
        self.index.clear();
        self.tools.clear();
        self.live = None;
        self.open = None;
        self.replaying = true;
    }

    /// The replay is over: settle the last replayed message and put the held prompts back
    /// at the end, where they belong in time. They are not re-emitted — the client has
    /// rendered them since the moment they were typed.
    pub fn end_replay(&mut self, out: &mut Vec<Emit>) {
        self.replaying = false;
        self.finish_turn(out);
        for message in std::mem::take(&mut self.held) {
            if let Some(id) = message.info["id"].as_str() {
                self.index.insert(id.to_string(), self.messages.len());
            }
            self.messages.push(message);
        }
    }

    /// A user prompt. opman renders the prompt it sent optimistically, but the agent also
    /// replays user messages on `session/load`, so this is what rebuilds history.
    pub fn user_message(&mut self, prompt: &Prompt, out: &mut Vec<Emit>) -> String {
        // Replayed history is finished business, so a user turn ends the assistant turn
        // before it outright. Live, it only detaches: an agent that accepts a follow-up
        // mid-turn is still generating, and stamping it complete would settle tools that
        // are genuinely running.
        if self.replaying {
            self.finish_turn(out);
        }
        self.user_turn += 1;
        let mid = format!("msg_user_{}_{}", self.session_id, self.user_turn);
        let info = json!({
            "role": "user",
            "id": mid,
            "sessionID": self.session_id,
            "time": { "created": super::now_ms() },
        });
        // The attachments ride along as `file` parts, so the user's own bubble shows what
        // they sent rather than only the words that came with it.
        let parts = prompt.message_parts(&mid, &self.session_id);
        self.index.insert(mid.clone(), self.messages.len());
        self.messages.push(MsgOut {
            info: info.clone(),
            parts: parts.clone(),
        });
        // A user turn closes any assistant message that was still streaming.
        self.live = None;
        self.open = None;
        out.push(Emit::Message(info));
        for part in parts {
            out.push(Emit::Part(part));
        }
        mid
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
                    append_text(part, text);
                    out.push(Emit::Delta {
                        session_id: self.session_id.clone(),
                        message_id: mid,
                        part_id: part_id(part),
                        delta: text.to_string(),
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
