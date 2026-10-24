//! ACP tool-call frames → opencode tool parts.
//!
//! The payoff for mapping onto the shape the shared transcript parser already produces is
//! that every tool renderer opman has (Bash, Edit, Write, Read, TodoWrite, Task …) keeps
//! working with no frontend change: `tool` carries the agent's real tool name and
//! `state.input` carries its native arguments. Agents that don't name their tools (Codex)
//! fall back to the ACP `kind`, which `tool_kind` turns into the card that renders that
//! category, with the card's input filled from the frame; `tool_output` turns terminal
//! chunks and exit statuses into text.
//!
//! Naming an ACP tool call takes three sources, because agents disagree about where the
//! name goes. Claude's adapter puts it in `_meta`. Others — opencode among them — put it in
//! the *opening* `tool_call`'s `title` and then overwrite `title` on every later
//! `tool_call_update` with a human description of that particular call ("bash" becomes the
//! command, "edit" becomes the path). So the opening title is identity and every later one
//! is prose, which is why the frame type decides whether it may be read as a name.

use serde_json::{json, Value};

use self::name::{is_generated_title, parsed_raw_input, tool_name};
use super::tool_kind::{fill_input, Shape};
use super::tool_output::{absorb_terminal, exit_code, finish, merge_output};

#[path = "tool_name.rs"]
mod name;

/// Merge one `tool_call` / `tool_call_update` payload into an existing tool part.
/// Absent fields are left alone — updates are deltas, not replacements.
pub fn merge(part: &mut Value, update: &Value) {
    let known = part
        .get("tool")
        .and_then(Value::as_str)
        .is_some_and(|name| name != super::emit::UNNAMED_TOOL);
    let named = tool_name(update, known);
    // A title that was just adopted as the tool's name (or is Antigravity's auto-generated
    // "Running <tool>" / "Run <tool>?" wrapper) is not also a description of the call;
    // storing it would print the tool name twice in the card header.
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty() && !is_generated_title(t, named.as_deref()));
    if let Some(name) = named {
        part["tool"] = Value::String(name);
    }
    let shape = Shape::of(part.get("tool").and_then(Value::as_str).unwrap_or_default());
    let Some(state) = part.get_mut("state").and_then(Value::as_object_mut) else {
        return;
    };

    // `rawInput` grows as the agent streams the call's arguments, so the last one wins.
    // Antigravity's historical replay sends `rawInput` as a JSON string (`arguments_json`).
    if let Some(input) = parsed_raw_input(update) {
        let keep_existing = input.as_object().is_some_and(|o| o.is_empty())
            && state
                .get("input")
                .and_then(Value::as_object)
                .is_some_and(|o| !o.is_empty());
        if !keep_existing {
            state.insert("input".into(), input);
        }
    }
    merge_output(state, update);
    fill_input(state, shape, update);
    if let Some(title) = title {
        state.insert("title".into(), Value::String(title.to_string()));
    } else if !state.contains_key("title") {
        if let Some(prompt) = update
            .pointer("/_meta/prompt")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
        {
            state.insert("title".into(), Value::String(prompt.to_string()));
        }
    }
    absorb_terminal(state, update);
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
    finish(state);
    if mapped == "error" {
        // The UI reads `state.error`. A command's exit code says why without repeating the
        // output the card already shows; otherwise ACP puts the reason in the output.
        let reason = match exit_code(state) {
            Some(code) => format!("Exit code {code}"),
            None => state
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or("tool call failed")
                .to_string(),
        };
        state.insert("error".into(), Value::String(reason));
    }
}

/// Carry the ACP-only details (diffs, touched files, category) that have no opencode field
/// of their own into `state.metadata`, where the UI can surface them.
fn merge_metadata(state: &mut serde_json::Map<String, Value>, update: &Value) {
    let mut pending: Vec<(&str, Value)> = Vec::new();
    let blocks = update.get("content").and_then(Value::as_array);
    let raw = update.get("rawOutput");
    if let Some(structured) =
        raw.and_then(|raw| raw.get("result").unwrap_or(raw).get("structuredContent"))
    {
        pending.push(("structuredContent", structured.clone()));
    }
    let parent_keys = ["parentCallID", "parentToolCallId", "parent_tool_call_id"];
    if let Some(parent) = [Some(update), update.get("_meta")]
        .into_iter()
        .flatten()
        .find_map(|source| parent_keys.iter().find_map(|key| source.get(key)?.as_str()))
        .filter(|parent| !parent.is_empty())
    {
        pending.push(("parentCallID", Value::String(parent.to_string())));
    }
    let mcp_blocks = raw
        .and_then(|raw| raw.get("result").unwrap_or(raw).get("content"))
        .and_then(Value::as_array);
    let preview = blocks
        .into_iter()
        .flatten()
        .map(|block| block.get("content").unwrap_or(block))
        .chain(mcp_blocks.into_iter().flatten())
        .filter_map(super::content::render)
        .find_map(|rendered| match rendered {
            super::content::Rendered::File { url, .. } => Some(Value::String(url)),
            super::content::Rendered::Text(_) => None,
        });
    if let Some(preview) = preview {
        pending.push(("preview", preview));
    }
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
    finish(state);
    match state.get_mut("time").and_then(Value::as_object_mut) {
        Some(time) => time.insert("end".into(), json!(ts)),
        None => state.insert("time".into(), json!({ "start": ts, "end": ts })),
    };
    true
}

fn is_type(value: &Value, expected: &str) -> bool {
    value.get("type").and_then(Value::as_str) == Some(expected)
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tool_tests;

#[cfg(test)]
#[path = "tool_nested_tests.rs"]
mod nested_tests;
