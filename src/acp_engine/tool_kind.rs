//! ACP `kind` → the opman tool a call renders as, and the input shape that tool reads.
//!
//! Agents that don't name their tools (Codex among them) send only a category, a prose
//! title, locations and diffs. opman's cards key on tool names and read native argument
//! shapes (`command`, `file_path`, `old_string` …), so a category call is given the name of
//! the card that renders it, and its input is filled from what the frame did carry. Native
//! fields that are already present are never touched, so a named call passes through as is.

use serde_json::{json, Map, Value};

/// The ACP tool categories. `other` is absent on purpose: it says nothing about the call.
#[derive(Clone, Copy)]
pub enum Kind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    SwitchMode,
}

impl Kind {
    pub fn of(update: &Value) -> Option<Self> {
        Some(match update.get("kind")?.as_str()? {
            "read" => Self::Read,
            "edit" => Self::Edit,
            "delete" => Self::Delete,
            "move" => Self::Move,
            "search" => Self::Search,
            "execute" => Self::Execute,
            "think" => Self::Think,
            "fetch" => Self::Fetch,
            "switch_mode" => Self::SwitchMode,
            _ => return None,
        })
    }

    /// The opman tool that renders a call of this kind. The frame's content decides between
    /// close cards: a read with no file is a listing, an edit that only adds files is a write,
    /// and a search that carries a query is a web search.
    pub fn tool_name(self, update: &Value) -> &'static str {
        match self {
            Self::Execute => "bash",
            Self::Read if first_location(update).is_some() => "read",
            Self::Read => "ls",
            Self::Edit if only_new_files(update) => "write",
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Move => "move",
            Self::Search if raw_input_has(update, "query") => "websearch",
            Self::Search => "grep",
            Self::Fetch => "webfetch",
            Self::Think => "think",
            Self::SwitchMode => "switch_mode",
        }
    }
}

/// Which input a tool's card reads, so it can be filled from frames that lack it.
#[derive(Clone, Copy)]
pub enum Shape {
    /// `mcp__<server>__<tool>`: the card reads the MCP arguments themselves.
    Mcp,
    /// Read / LS: a `file_path`.
    File,
    /// Edit / Write: a `file_path` plus the before and after text.
    Diff,
    /// Anything else, whose input is used as the agent sent it.
    Native,
}

impl Shape {
    pub fn of(tool: &str) -> Self {
        match tool {
            "read" | "ls" => Self::File,
            "edit" | "write" => Self::Diff,
            mcp if mcp.starts_with("mcp__") => Self::Mcp,
            _ => Self::Native,
        }
    }
}

/// Fill `state.input` with the fields the tool's card reads, from what this frame carried.
pub fn fill_input(state: &mut Map<String, Value>, shape: Shape, update: &Value) {
    if matches!(shape, Shape::Native) {
        return;
    }
    let input = state.entry("input").or_insert_with(|| json!({}));
    let Some(fields) = input.as_object_mut() else {
        return;
    };
    if let Shape::Mcp = shape {
        // Codex wraps an MCP call's arguments as `{server, tool, arguments}`.
        if fields.contains_key("server") {
            if let Some(arguments) = fields.remove("arguments") {
                *input = arguments;
            }
        }
        return;
    }
    if ["file_path", "filePath", "path"]
        .iter()
        .any(|key| fields.contains_key(*key))
    {
        return;
    }
    if let Shape::Diff = shape {
        fill_from_diffs(fields, update);
        return;
    }
    if let Some(path) = first_location(update) {
        fields.insert("file_path".into(), path.clone());
    }
}

/// Turn ACP diff blocks into Edit / Write / MultiEdit arguments.
fn fill_from_diffs(fields: &mut Map<String, Value>, update: &Value) {
    let diffs: Vec<&Value> = diffs(update).collect();
    let Some(first) = diffs.first() else {
        return;
    };
    if let Some(path) = first.get("path") {
        fields.insert("file_path".into(), path.clone());
    }
    if diffs.len() > 1 {
        let edits = diffs.into_iter().map(hunk).collect();
        fields.insert("edits".into(), Value::Array(edits));
        return;
    }
    if is_new_file(first) {
        fields.insert("content".into(), text(first, "newText"));
        return;
    }
    fields.insert("old_string".into(), text(first, "oldText"));
    fields.insert("new_string".into(), text(first, "newText"));
}

fn hunk(diff: &Value) -> Value {
    json!({
        "file_path": diff.get("path"),
        "old_string": text(diff, "oldText"),
        "new_string": text(diff, "newText"),
    })
}

/// A diff field as a string; a new file's `oldText` is null.
fn text(diff: &Value, key: &str) -> Value {
    match diff.get(key) {
        Some(Value::String(s)) => Value::String(s.clone()),
        _ => Value::String(String::new()),
    }
}

fn diffs(update: &Value) -> impl Iterator<Item = &Value> {
    update
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|c| c.get("type").and_then(Value::as_str) == Some("diff"))
}

fn is_new_file(diff: &Value) -> bool {
    diff.get("oldText").is_none_or(Value::is_null)
}

fn only_new_files(update: &Value) -> bool {
    let mut seen = false;
    for diff in diffs(update) {
        if !is_new_file(diff) {
            return false;
        }
        seen = true;
    }
    seen
}

fn first_location(update: &Value) -> Option<&Value> {
    update.get("locations")?.as_array()?.first()?.get("path")
}

fn raw_input_has(update: &Value, key: &str) -> bool {
    update
        .get("rawInput")
        .is_some_and(|input| input.get(key).is_some())
}

#[cfg(test)]
#[path = "tool_kind_tests.rs"]
mod tool_kind_tests;
