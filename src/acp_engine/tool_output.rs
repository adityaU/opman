//! A tool call's output as the text opman renders.
//!
//! Agents report output three ways: a `rawOutput` string or object, text `content` blocks,
//! and — for a command the agent ran itself (Codex) — chunks under `_meta` naming a terminal
//! opman never created. Chunks stream into `state.metadata.output`, which the Bash card
//! already shows while a call runs, and move to `state.output` when the call ends.

use serde_json::{json, Map, Value};

/// The chunk channels an agent streams a command's output on. `terminal_output` is a whole
/// snapshot; `terminal_output_delta` appends.
enum Chunk<'a> {
    Snapshot(&'a str),
    Append(&'a str),
}

impl<'a> Chunk<'a> {
    fn of(meta: &'a Value) -> Option<Self> {
        let data = |key: &str| meta.get(key)?.get("data")?.as_str();
        data("terminal_output_delta")
            .map(Self::Append)
            .or_else(|| data("terminal_output").map(Self::Snapshot))
    }
}

/// The text of `rawOutput`, else of the text content blocks. An exit-status object is not
/// output, and an MCP result is unwrapped to its text.
pub fn raw_output(update: &Value) -> Option<String> {
    if let Some(raw) = update.get("rawOutput").filter(|raw| !raw.is_null()) {
        if let Some(text) = raw.as_str() {
            return Some(text.to_string());
        }
        if exit_status(raw).is_some() {
            return None;
        }
        if let Some(text) = mcp_text(raw) {
            return Some(text);
        }
        return serde_json::to_string(raw).ok();
    }
    let items = update.get("content")?.as_array()?;
    let joined = items
        .iter()
        .filter(|c| c.get("type").and_then(Value::as_str) == Some("content"))
        .filter_map(|c| c.get("content")?.get("text")?.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.is_empty()).then_some(joined)
}

/// Fold terminal chunks and the exit code into `state.metadata`.
pub fn absorb_terminal(state: &mut Map<String, Value>, update: &Value) {
    let meta = update.get("_meta");
    let chunk = meta.and_then(Chunk::of);
    let exit = meta
        .and_then(|m| m.get("terminal_exit")?.get("exit_code")?.as_i64())
        .or_else(|| update.get("rawOutput").and_then(exit_status));
    if chunk.is_none() && exit.is_none() {
        return;
    }
    let Some(metadata) = state
        .entry("metadata")
        .or_insert_with(|| json!({}))
        .as_object_mut()
    else {
        return;
    };
    if let Some(code) = exit {
        metadata.insert("exit".into(), json!(code));
    }
    let Some(chunk) = chunk else { return };
    if let (Chunk::Append(data), Some(Value::String(live))) = (&chunk, metadata.get_mut("output")) {
        live.push_str(data);
        return;
    }
    let (Chunk::Append(data) | Chunk::Snapshot(data)) = chunk;
    metadata.insert("output".into(), Value::String(data.to_string()));
}

/// At the end of a call, move streamed output into `state.output` unless the agent sent a
/// final output of its own. Either way the live copy is dropped rather than kept twice.
pub fn finish(state: &mut Map<String, Value>) {
    let live = state
        .get_mut("metadata")
        .and_then(Value::as_object_mut)
        .and_then(|m| m.remove("output"));
    let Some(live) = live else { return };
    let has_output = state
        .get("output")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    if !has_output {
        state.insert("output".into(), live);
    }
}

/// The exit code a finished command reported, if it was one.
pub fn exit_code(state: &Map<String, Value>) -> Option<i64> {
    state.get("metadata")?.get("exit")?.as_i64()
}

/// `{"exit_code": n}` (or `exitCode`) alone: a status, not something to print.
fn exit_status(raw: &Value) -> Option<i64> {
    let fields = raw.as_object().filter(|o| o.len() == 1)?;
    fields
        .get("exit_code")
        .or_else(|| fields.get("exitCode"))?
        .as_i64()
}

/// An MCP `CallToolResult` (`{result: {content: [...]}, error}`) as its text.
fn mcp_text(raw: &Value) -> Option<String> {
    if let Some(error) = raw.get("error").filter(|e| !e.is_null()) {
        return Some(match error.get("message").and_then(Value::as_str) {
            Some(message) => message.to_string(),
            None => error.to_string(),
        });
    }
    let blocks = raw.get("result")?.get("content")?.as_array()?;
    let joined = blocks
        .iter()
        .filter_map(|b| b.get("text")?.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Some(joined)
}

#[cfg(test)]
#[path = "tool_output_tests.rs"]
mod tool_output_tests;
