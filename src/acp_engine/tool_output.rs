//! A tool call's output as the text opman renders.
//!
//! Agents report output three ways: a `rawOutput` string or object, text `content` blocks,
//! and — for a command the agent ran itself (Codex) — chunks under `_meta` naming a terminal
//! opman never created. Chunks stream into `state.metadata.output`, which the Bash card
//! already shows while a call runs, and move to `state.output` when the call ends.

use serde_json::{json, Map, Value};

#[path = "tool_output_result.rs"]
mod result;

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

/// The text of `rawOutput`, else of the text content blocks, else of Antigravity's
/// localharness step fields in `rawInput`. An exit-status object is not output, and an MCP
/// or command result is unwrapped to its text.
pub fn raw_output(update: &Value) -> Option<String> {
    if let Some(raw) = update.get("rawOutput").filter(|raw| !raw.is_null()) {
        if let Some(text) = raw.as_str() {
            return Some(text.to_string());
        }
        if exit_status(raw).is_some() {
            return None;
        }
        if let Some(cmd_out) = command_output(raw) {
            return (!cmd_out.is_empty()).then_some(cmd_out);
        }
        if let Some(text) = result::mcp_text(raw) {
            return Some(text);
        }
        if result::is_call_log(raw) {
            return None;
        }
        if let Some(text) = content_text(update) {
            return Some(text);
        }
        return serde_json::to_string(raw).ok();
    }
    content_text(update).or_else(|| local_step_output(update))
}

fn content_text(update: &Value) -> Option<String> {
    let items = update.get("content")?.as_array()?;
    let joined = items
        .iter()
        .filter(|c| c.get("type").and_then(Value::as_str) == Some("content"))
        .filter_map(|c| c.get("content")?.get("text")?.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.is_empty()).then_some(joined)
}

/// Antigravity's `run_command` / `shell` emits `{commandLine, workingDir, exitCode, combinedOutput, formatted_output}`.
fn command_output(raw: &Value) -> Option<String> {
    let obj = raw.as_object()?;
    if !obj.contains_key("combinedOutput") && !obj.contains_key("formatted_output") {
        return None;
    }
    let text = obj
        .get("combinedOutput")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| obj.get("formatted_output").and_then(Value::as_str))
        .unwrap_or_default();
    Some(text.to_string())
}

/// Antigravity's Go `localharness` places tool results inside the step dict (`call.args` /
/// `rawInput`) with an empty `content` array on `view_file`, `list_directory`, `find_file`,
/// `search_directory`, `read_url_content`, and `run_workflow`.
fn local_step_output(update: &Value) -> Option<String> {
    let raw = update.get("rawInput")?;
    let parsed;
    let obj = match raw {
        Value::Object(map) => map,
        Value::String(s) => {
            parsed = serde_json::from_str::<Value>(s).ok()?;
            parsed.as_object()?
        }
        _ => return None,
    };
    let kind = update.get("kind").and_then(Value::as_str).unwrap_or("");
    let title = update.get("title").and_then(Value::as_str).unwrap_or("");

    // `view_file` / `read_file`: `content` in `rawInput` when kind is `read` or title names view/read.
    if kind == "read" || title.contains("view_file") || title.contains("read_file") {
        if let Some(content) = obj
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return Some(content.to_string());
        }
    }
    // `list_directory`: `entries` or `results` array of `{name, is_directory, ...}`.
    if kind == "read" || title.contains("list_dir") {
        if let Some(entries) = obj
            .get("entries")
            .or_else(|| obj.get("results"))
            .and_then(Value::as_array)
        {
            let lines: Vec<String> = entries
                .iter()
                .filter_map(|e| {
                    if let Some(s) = e.as_str() {
                        return Some(s.to_string());
                    }
                    let name = e
                        .get("name")
                        .or_else(|| e.get("path"))
                        .and_then(Value::as_str)?;
                    let is_dir = e
                        .get("is_directory")
                        .or_else(|| e.get("isDirectory"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                        || e.get("kind").and_then(Value::as_str) == Some("dir");
                    Some(if is_dir && !name.ends_with('/') {
                        format!("{name}/")
                    } else {
                        name.to_string()
                    })
                })
                .collect();
            if !lines.is_empty() {
                return Some(lines.join("\n"));
            }
        }
    }
    // `find_file` / `search_directory` / `run_workflow`: `output`, `files`, or `results`.
    if let Some(out) = obj
        .get("output")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(out.to_string());
    }
    if let Some(files) = obj.get("files").and_then(Value::as_array) {
        let lines: Vec<&str> = files.iter().filter_map(Value::as_str).collect();
        if !lines.is_empty() {
            return Some(lines.join("\n"));
        }
    }
    if kind == "search" {
        if let Some(results) = obj.get("results").and_then(Value::as_array) {
            let lines: Vec<String> = results
                .iter()
                .filter_map(|r| {
                    if let Some(s) = r.as_str() {
                        return Some(s.to_string());
                    }
                    let path = r
                        .get("path")
                        .or_else(|| r.get("file"))
                        .or_else(|| r.get("file_path"))
                        .and_then(Value::as_str)?;
                    if let Some(line) = r
                        .get("line")
                        .or_else(|| r.get("line_number"))
                        .and_then(Value::as_u64)
                    {
                        let text = r
                            .get("text")
                            .or_else(|| r.get("content"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        Some(format!("{path}:{line}:{text}"))
                    } else {
                        Some(path.to_string())
                    }
                })
                .collect();
            if !lines.is_empty() {
                return Some(lines.join("\n"));
            }
        }
    }
    if kind == "fetch" || title.contains("read_url_content") || title.contains("url_context") {
        if let Some(summary) = obj
            .get("summary")
            .or_else(|| obj.get("content"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return Some(summary.to_string());
        }
    }
    None
}

/// Fold terminal chunks and the exit code into `state.metadata`.
pub fn absorb_terminal(state: &mut Map<String, Value>, update: &Value) {
    let meta = update.get("_meta");
    let chunk = meta.and_then(Chunk::of);
    let exit = meta
        .and_then(|m| m.get("terminal_exit")?.get("exit_code")?.as_i64())
        .or_else(|| update.get("rawOutput").and_then(exit_status))
        .or_else(|| update.get("rawOutput").and_then(command_exit_code));
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

/// Select output before terminal metadata is absorbed. Native rawOutput remains authoritative
/// across later content-only frames, including frames received after completion.
pub(super) fn merge_output(state: &mut Map<String, Value>, update: &Value) {
    result::merge_output(state, update);
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

fn command_exit_code(raw: &Value) -> Option<i64> {
    let fields = raw.as_object()?;
    fields
        .get("exit_code")
        .or_else(|| fields.get("exitCode"))?
        .as_i64()
}

#[cfg(test)]
#[path = "tool_output_tests.rs"]
mod tool_output_tests;

#[cfg(test)]
#[path = "tool_output_result_tests.rs"]
mod result_tests;
