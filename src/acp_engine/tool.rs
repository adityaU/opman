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

use super::tool_kind::{fill_input, Kind, Shape};
use super::tool_output::{absorb_terminal, exit_code, finish, raw_output};

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
        .is_some_and(|name| name != super::emit::UNNAMED_TOOL);
    let named = tool_name(update, known);
    // A title that was just adopted as the tool's name is not also a description of the
    // call; storing it would print the same word twice in the card header.
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| named.as_deref() != Some(*title));
    if let Some(name) = named {
        part["tool"] = Value::String(name);
    }
    let shape = Shape::of(part.get("tool").and_then(Value::as_str).unwrap_or_default());
    let Some(state) = part.get_mut("state").and_then(Value::as_object_mut) else {
        return;
    };

    // `rawInput` grows as the agent streams the call's arguments, so the last one wins.
    if let Some(input) = update.get("rawInput").filter(|input| !input.is_null()) {
        state.insert("input".into(), input.clone());
    }
    fill_input(state, shape, update);
    if let Some(title) = title {
        state.insert("title".into(), Value::String(title.to_string()));
    }
    if let Some(output) = raw_output(update) {
        state.insert("output".into(), Value::String(output));
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

/// The agent's own tool name, else the opening frame's title, else the ACP category, else
/// nothing (leaving whatever the part already had).
///
/// `known` says whether this part already has a name: `kind` is a category rather than a
/// name, so it must never displace a real one.
fn tool_name(update: &Value, known: bool) -> Option<String> {
    let claude = CLAUDE_TOOL_NAME
        .iter()
        .try_fold(update, |node, key| node.get(key))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    if let Some(name) = claude {
        return Some(canonical(name));
    }
    if let Some(name) = mcp_name(update) {
        return Some(name);
    }
    if is_opening(update) {
        if let Some(title) = update
            .get("title")
            .and_then(Value::as_str)
            .filter(|title| is_tool_name(title) && !is_its_command(update, title))
        {
            return Some(canonical(title));
        }
    }
    if known {
        return None;
    }
    Kind::of(update).map(|kind| kind.tool_name(update).to_string())
}

/// Codex flags an MCP call in `_meta` and names it `{server, tool}` in the input. opman's
/// MCP cards key on Claude's `mcp__<server>__<tool>` spelling, so it is given that name.
fn mcp_name(update: &Value) -> Option<String> {
    let meta = update.get("_meta")?.get("is_mcp_tool_call")?;
    if meta.as_bool() != Some(true) {
        return None;
    }
    let input = update.get("rawInput")?;
    let server = input.get("server")?.as_str()?;
    let tool = input.get("tool")?.as_str()?;
    Some(format!("mcp__{server}__{tool}"))
}

/// A title that is the very command the call runs is prose, however much it looks like a
/// name: Codex titles `pwd` as `pwd`.
fn is_its_command(update: &Value, title: &str) -> bool {
    update
        .get("rawInput")
        .and_then(|input| input.get("command"))
        .and_then(Value::as_str)
        == Some(title)
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

fn is_type(value: &Value, expected: &str) -> bool {
    value.get("type").and_then(Value::as_str) == Some(expected)
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tool_tests;
