//! The event vocabulary the transcript speaks, and the small helpers that build parts.
//!
//! Split from [`super::transcript`] so the folding logic there reads as one story: what an
//! update does to the conversation, not how a part is shaped.

use serde_json::{json, Value};

/// One event the caller should broadcast. Kept separate from emission so the transcript
/// stays a pure data structure (and directly testable).
#[derive(Debug, Clone, PartialEq)]
pub enum Emit {
    /// `message.updated` — the message envelope (role, model, tokens, cost).
    Message(Value),
    /// `message.part.updated` — a whole part, for creation and for non-text changes.
    Part(Value),
    /// `message.part.delta` — append text to an existing part. One token, one small
    /// frame, instead of resending everything accumulated so far.
    Delta {
        session_id: String,
        message_id: String,
        part_id: String,
        delta: String,
    },
}

/// Which opencode part type a streamed chunk lands in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Chunk {
    Text,
    Reasoning,
}

impl Chunk {
    pub(super) fn part_type(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Reasoning => "reasoning",
        }
    }
}

/// Append in place. Mutating the existing `String` avoids reallocating the whole accumulated
/// text on every token, which matters when a long reply arrives one token at a time.
pub(super) fn append_text(part: &mut Value, text: &str) {
    if let Some(Value::String(existing)) = part.get_mut("text") {
        existing.push_str(text);
        return;
    }
    part["text"] = Value::String(text.to_string());
}

pub(super) fn part_id(part: &Value) -> String {
    part.get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Placeholder `tool` for a part whose call has not named itself yet. Distinguishable from
/// a real name so a later frame can still claim the slot.
pub(super) const UNNAMED_TOOL: &str = "tool";

/// A fresh tool part. The id is the ACP call id (not `{message}:{index}`) so every later
/// `tool_call_update` addresses the same part; ordering comes from the array position.
pub(super) fn new_tool_part(session_id: &str, message_id: &str, call_id: &str) -> Value {
    json!({
        "type": "tool",
        "id": call_id,
        "callID": call_id,
        "tool": UNNAMED_TOOL,
        "messageID": message_id,
        "sessionID": session_id,
        "state": { "status": "running", "input": {}, "time": { "start": super::now_ms() } },
    })
}
