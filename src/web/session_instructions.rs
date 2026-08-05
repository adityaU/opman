//! Session instructions: the standing guidance a session opens with.
//!
//! These are the items the UI calls "session instructions" (stored as personal
//! memory, scoped global / project / session). They used to be prepended by the
//! browser to *every* outgoing message, which meant the same paragraph was
//! re-sent and re-billed on every turn and drifted whenever a different client
//! sent the message. They are assembled here instead, and delivered once: on a
//! session's first turn, and on the handoff message that opens a session taken
//! over from another runner.
//!
//! One formatter, used by every path that opens a session — chat, kanban task
//! launches, and pipeline stages — so the wording can only be changed in one
//! place.

use crate::web::types::PersonalMemoryItem;

/// Opening marker of the instruction block.
pub const INSTRUCTIONS_MARKER: &str = "[Session instructions]";

/// Marker that ends the block and begins the user's own text.
pub const REQUEST_MARKER: &str = "[User request]";

/// Marker used before the rename. Still recognised when reading old
/// transcripts; never written.
pub const LEGACY_MARKER: &str = "[Assistant memory in effect]";

/// Prefix `text` with the active session instructions.
///
/// Returns `text` unchanged when there are no instructions, or when the text
/// already carries a block — a queued message or a retry can arrive
/// pre-wrapped, and wrapping twice would spend the tokens twice.
pub fn wrap(text: &str, instructions: &[PersonalMemoryItem]) -> String {
    if instructions.is_empty() || carries_block(text) {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len() + 128);
    out.push_str(INSTRUCTIONS_MARKER);
    out.push('\n');
    for item in instructions {
        out.push_str("- ");
        out.push_str(&item.label);
        out.push_str(": ");
        out.push_str(&item.content);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(REQUEST_MARKER);
    out.push('\n');
    out.push_str(text);
    out
}

/// Whether `text` already opens with an instruction block, current or legacy.
pub fn carries_block(text: &str) -> bool {
    let start = text.trim_start();
    start.starts_with(INSTRUCTIONS_MARKER) || start.starts_with(LEGACY_MARKER)
}

/// Prefix the first text part of a message body in place.
///
/// Only the first text part is touched: the instructions belong at the very top
/// of the turn, and later parts are attachments or continuation text that must
/// keep their own order. Returns whether anything was written, so the caller
/// only records delivery when delivery actually happened.
pub fn apply_to_parts(
    parts: &mut [serde_json::Value],
    instructions: &[PersonalMemoryItem],
) -> bool {
    if instructions.is_empty() {
        return false;
    }
    for part in parts.iter_mut() {
        let is_text = part
            .get("type")
            .and_then(|t| t.as_str())
            .map_or(true, |t| t == "text");
        if !is_text {
            continue;
        }
        let Some(text) = part.get("text").and_then(|t| t.as_str()) else {
            continue;
        };
        let wrapped = wrap(text, instructions);
        if wrapped == text {
            return false;
        }
        part["text"] = serde_json::Value::String(wrapped);
        return true;
    }
    false
}

#[cfg(test)]
#[path = "session_instructions_tests.rs"]
mod session_instructions_tests;
