//! The message that carries a session across runners.
//!
//! Switching runner mid-session mints a new session on the target runner, and
//! that session's first turn has to carry everything the old runner knew. It
//! used to carry a flattened `role: text` *summary* pasted into prose, sent as
//! the user's own message — so the UI showed the whole transcript inside the
//! user's bubble, and a second switch summarised that bubble again, nesting one
//! copy of the history inside the next.
//!
//! Now the real transcript is rendered once, fenced by markers the UI knows to
//! collapse, and the user's actual text follows it untouched. Rendering strips
//! any handoff or session-instruction block it finds in a historical message,
//! so chained handoffs stay flat and the instructions are never repeated.
use serde_json::Value;

use crate::web::session_instructions::{INSTRUCTIONS_MARKER, LEGACY_MARKER, REQUEST_MARKER};

/// Opening marker of the handoff block. Everything up to [`HANDOFF_END_MARKER`]
/// is prior context, not something the user typed.
pub const HANDOFF_MARKER: &str = "[Handoff transcript]";

/// Marker that closes the block. The user's own text follows it.
pub const HANDOFF_END_MARKER: &str = "[End handoff transcript]";

/// Written in place of the messages dropped to stay under the byte budget.
const TRUNCATION_NOTICE: &str = "[Earlier transcript omitted]";

/// Byte budget for the rendered transcript. Generous — the point of the handoff
/// is that the new runner has the context — but bounded, because a long session
/// would otherwise be one unbillable-by-accident megaprompt.
const MAX_TRANSCRIPT: usize = 24_000;

/// One turn of the prior conversation.
struct Entry {
    role: String,
    text: String,
}

/// Messages in send order. `messages()` returns an array on some runners and a
/// map keyed by id on others, and a map has no order worth trusting, so sort by
/// creation time in both cases.
fn messages_in_order(body: &Value) -> Vec<Value> {
    let mut messages: Vec<Value> = if let Some(array) = body.as_array() {
        array.clone()
    } else if let Some(object) = body.as_object() {
        object.values().cloned().collect()
    } else {
        return Vec::new();
    };
    messages.sort_by_key(|message| {
        message
            .pointer("/info/time/created")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    });
    messages
}

/// Drop the parts of a historical message that this function itself wrote on an
/// earlier handoff, plus any session-instruction block.
///
/// Without this, every switch would quote the previous switch's whole block:
/// the transcript would grow by a full copy of itself each time.
fn strip_wrappers(text: &str) -> &str {
    let mut out = text.trim();
    if out.starts_with(HANDOFF_MARKER) {
        out = match out.find(HANDOFF_END_MARKER) {
            Some(end) => out[end + HANDOFF_END_MARKER.len()..].trim_start(),
            // An unterminated block is all context and no user text.
            None => "",
        };
    }
    if out.starts_with(INSTRUCTIONS_MARKER) || out.starts_with(LEGACY_MARKER) {
        if let Some(idx) = out.find(REQUEST_MARKER) {
            out = out[idx + REQUEST_MARKER.len()..].trim_start();
        }
    }
    out
}

/// Join every text part of one message.
fn message_text(message: &Value) -> String {
    message
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn entries(body: &Value) -> Vec<Entry> {
    let mut entries = Vec::new();
    for message in messages_in_order(body) {
        let role = message
            .pointer("/info/role")
            .and_then(Value::as_str)
            .unwrap_or("message");
        let text = message_text(&message);
        let stripped = strip_wrappers(&text);
        if stripped.is_empty() {
            continue;
        }
        entries.push(Entry {
            role: role.to_string(),
            text: stripped.to_string(),
        });
    }
    entries
}

/// Render the prior conversation as labelled turns, newest kept first when the
/// budget forces a cut — the tail of a session is what the next runner needs.
pub fn render_transcript(body: &Value) -> String {
    let entries = entries(body);
    if entries.is_empty() {
        return "No transcript was available.".to_string();
    }

    let mut kept: Vec<String> = Vec::with_capacity(entries.len());
    let mut budget = MAX_TRANSCRIPT;
    let mut dropped = false;
    for entry in entries.iter().rev() {
        let block = format!("--- {} ---\n{}", entry.role, entry.text);
        if block.len() + 1 > budget {
            dropped = true;
            break;
        }
        budget -= block.len() + 1;
        kept.push(block);
    }
    // A single turn larger than the whole budget would otherwise leave nothing
    // but the notice. Keep its tail — the end of a message is where the ask is.
    if kept.is_empty() {
        let last = entries.last().expect("non-empty");
        let tail = last.text.len().saturating_sub(MAX_TRANSCRIPT);
        kept.push(format!(
            "--- {} ---\n{}",
            last.role,
            &last.text[floor_char(&last.text, tail)..]
        ));
    }
    kept.reverse();

    let mut out = String::with_capacity(MAX_TRANSCRIPT - budget + 64);
    if dropped {
        out.push_str(TRUNCATION_NOTICE);
        out.push_str("\n\n");
    }
    out.push_str(&kept.join("\n\n"));
    out
}

/// Round `index` down to a char boundary so slicing a UTF-8 string cannot panic.
fn floor_char(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index.min(text.len())
}

/// Build the first message of the handed-over session: the fenced transcript,
/// then the user's text exactly as they sent it.
///
/// The user's text is appended verbatim and carries no marker of its own, so a
/// session-instructions block already wrapped around it keeps working — the UI
/// parses the two blocks in sequence.
pub fn build_prompt(from_runner: &str, transcript: &str, user_text: &str) -> String {
    let mut out = String::with_capacity(transcript.len() + user_text.len() + 256);
    out.push_str(HANDOFF_MARKER);
    out.push('\n');
    out.push_str("You are continuing a coding session handed over from the ");
    out.push_str(from_runner);
    out.push_str(" runner. The turns below are the conversation so far — treat them as your own history, and do not repeat work already done.\n\n");
    out.push_str(transcript);
    out.push('\n');
    out.push_str(HANDOFF_END_MARKER);
    out.push_str("\n\n");
    out.push_str(user_text);
    out
}

#[cfg(test)]
#[path = "runner_handoff_tests.rs"]
mod runner_handoff_tests;
