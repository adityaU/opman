//! ACP tool-call frames → opencode tool parts.
//!
//! The payoff for mapping onto the shape the shared transcript parser already produces is
//! that every tool renderer opman has (Bash, Edit, Write, Read, TodoWrite, Task …) keeps
//! working with no frontend change: `tool` carries the agent's real tool name and
//! `state.input` carries its native arguments. Agents that don't name their tools fall
//! back to the ACP `kind`, so a generic server still renders as a recognisable category.
//!
//! Naming an ACP tool call takes three sources, because agents disagree about where the
//! name goes. Claude's adapter puts it in `_meta`. Others — opencode among them — put it in
//! the *opening* `tool_call`'s `title` and then overwrite `title` on every later
//! `tool_call_update` with a human description of that particular call ("bash" becomes the
//! command, "edit" becomes the path). So the opening title is identity and every later one
//! is prose, which is why the frame type decides whether it may be read as a name.

use serde_json::{json, Value};

/// Claude's adapter reports the underlying tool name here; ACP itself has no field for it.
const CLAUDE_TOOL_NAME: [&str; 3] = ["_meta", "claudeCode", "toolName"];

/// The `sessionUpdate` discriminator of the frame that opens a call, as opposed to the
/// `tool_call_update` run that follows it.
const OPENING_FRAME: &str = "tool_call";

/// Longest plausible tool name. A title longer than this is prose, whatever it looks like.
const MAX_NAME_LEN: usize = 64;

/// Merge one `tool_call` / `tool_call_update` payload into an existing tool part.
/// Absent fields are left alone — updates are deltas, not replacements.
pub fn merge(part: &mut Value, update: &Value) {
    let known = part
        .get("tool")
        .and_then(Value::as_str)
        .filter(|name| *name != super::emit::UNNAMED_TOOL)
        .map(str::to_string);
    let named = tool_name(update, known.as_deref());
    if let Some(name) = &named {
        part["tool"] = Value::String(name.clone());
    }
    let Some(state) = part.get_mut("state").and_then(Value::as_object_mut) else {
        return;
    };

    // `rawInput` grows as the agent streams the call's arguments, so the last one wins.
    if let Some(input) = update.get("rawInput") {
        if !input.is_null() {
            state.insert("input".into(), input.clone());
        }
    }
    if let Some(title) = update.get("title").and_then(Value::as_str) {
        // A title that was just adopted as the tool's name is not also a description of
        // the call; storing it would print the same word twice in the card header.
        if named.as_deref() != Some(title) {
            state.insert("title".into(), Value::String(title.to_string()));
        }
    }
    if let Some(output) = raw_output(update) {
        state.insert("output".into(), Value::String(output));
    }
    if let Some(status) = update.get("status").and_then(Value::as_str) {
        apply_status(state, status);
    }
    merge_metadata(state, update);
}

/// Map the ACP execution status onto the three states opman renders, stamping an end time
/// once the call stops running.
fn apply_status(state: &mut serde_json::Map<String, Value>, status: &str) {
    let mapped = match status {
        // ACP distinguishes "created but not started" from "running"; opman's renderer has
        // one spinner for both.
        "pending" | "in_progress" => "running",
        "completed" => "completed",
        "failed" => "error",
        _ => return,
    };
    state.insert("status".into(), Value::String(mapped.to_string()));
    if mapped == "running" {
        return;
    }
    let ts = super::now_ms();
    match state.get_mut("time").and_then(Value::as_object_mut) {
        Some(time) => {
            time.insert("end".into(), json!(ts));
        }
        None => {
            state.insert("time".into(), json!({ "start": ts, "end": ts }));
        }
    }
    if mapped == "error" {
        // The UI reads `state.error`; ACP puts the reason in the output.
        let reason = state
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("tool call failed")
            .to_string();
        state.insert("error".into(), Value::String(reason));
    }
}

/// Carry the ACP-only details (diffs, touched files, category) that have no opencode field
/// of their own into `state.metadata`, where the UI can surface them.
fn merge_metadata(state: &mut serde_json::Map<String, Value>, update: &Value) {
    let mut pending: Vec<(&str, Value)> = Vec::new();
    if let Some(kind) = update.get("kind").and_then(Value::as_str) {
        pending.push(("kind", Value::String(kind.to_string())));
    }
    if let Some(locations) = update.get("locations").and_then(Value::as_array) {
        let paths: Vec<Value> = locations
            .iter()
            .filter_map(|l| l.get("path").cloned())
            .collect();
        if !paths.is_empty() {
            pending.push(("locations", Value::Array(paths)));
        }
    }
    if let Some(diff) = update
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|c| is_type(c, "diff")))
    {
        pending.push(("diff", diff.clone()));
    }
    if pending.is_empty() {
        return;
    }
    let metadata = state
        .entry("metadata".to_string())
        .or_insert_with(|| json!({}));
    let Some(metadata) = metadata.as_object_mut() else {
        return;
    };
    for (key, value) in pending {
        metadata.insert(key.to_string(), value);
    }
}

/// Force a still-running tool to a terminal state. Returns whether it changed, so the
/// caller only re-emits parts that actually moved.
pub fn settle(part: &mut Value, ts: u64) -> bool {
    if part.get("type").and_then(Value::as_str) != Some("tool") {
        return false;
    }
    let Some(state) = part.get_mut("state").and_then(Value::as_object_mut) else {
        return false;
    };
    if state.get("status").and_then(Value::as_str) != Some("running") {
        return false;
    }
    state.insert("status".into(), Value::String("completed".to_string()));
    match state.get_mut("time").and_then(Value::as_object_mut) {
        Some(time) => time.insert("end".into(), json!(ts)),
        None => state.insert("time".into(), json!({ "start": ts, "end": ts })),
    };
    true
}

/// The agent's own tool name, else the opening frame's title, else the ACP category, else
/// nothing (leaving whatever the part already had).
///
/// `known` is the name already resolved for this part, if any: `kind` is a category rather
/// than a name, so it must never displace a real one.
fn tool_name(update: &Value, known: Option<&str>) -> Option<String> {
    let claude = CLAUDE_TOOL_NAME
        .iter()
        .try_fold(update, |node, key| node.get(key))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    if let Some(name) = claude {
        return Some(canonical(name));
    }
    if is_opening(update) {
        if let Some(title) = update
            .get("title")
            .and_then(Value::as_str)
            .filter(|title| is_tool_name(title))
        {
            return Some(canonical(title));
        }
    }
    if known.is_some() {
        return None;
    }
    update
        .get("kind")
        .and_then(Value::as_str)
        .filter(|k| !k.is_empty() && *k != "other")
        .map(str::to_string)
}

fn is_opening(update: &Value) -> bool {
    update.get("sessionUpdate").and_then(Value::as_str) == Some(OPENING_FRAME)
}

/// Whether a title is shaped like a tool identifier rather than a sentence about the call.
/// Agents that put prose in the opening title ("Read package.json", "Running tests") fail
/// this and fall through to `kind`, so a name is only ever adopted when it can be one.
fn is_tool_name(title: &str) -> bool {
    !title.is_empty()
        && title.len() <= MAX_NAME_LEN
        && title
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// opman renders subagent launches as opencode's `task` tool, whatever the agent calls it.
fn canonical(name: &str) -> String {
    match name {
        "Agent" | "Task" => "task".to_string(),
        other => other.to_string(),
    }
}

/// Tool output as text: the agent's `rawOutput` when it is a string, else the text of its
/// content blocks, else a compact rendering of a structured result.
fn raw_output(update: &Value) -> Option<String> {
    if let Some(raw) = update.get("rawOutput") {
        if let Some(text) = raw.as_str() {
            return Some(text.to_string());
        }
        if !raw.is_null() {
            return serde_json::to_string(raw).ok();
        }
    }
    let items = update.get("content")?.as_array()?;
    let joined = items
        .iter()
        .filter(|c| is_type(c, "content"))
        .filter_map(|c| c.get("content")?.get("text")?.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.is_empty()).then_some(joined)
}

fn is_type(value: &Value, expected: &str) -> bool {
    value.get("type").and_then(Value::as_str) == Some(expected)
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tool_tests;
