//! ACP tool-name and title normalization.

use serde_json::Value;

use super::super::tool_kind::Kind;

const CLAUDE_TOOL_NAME: [&str; 3] = ["_meta", "claudeCode", "toolName"];
const OPENING_FRAME: &str = "tool_call";
const MAX_NAME_LEN: usize = 64;

/// Select a tool name from the frame, preserving a name already assigned to the part.
pub(super) fn tool_name(update: &Value, known: bool) -> Option<String> {
    let claude = CLAUDE_TOOL_NAME
        .iter()
        .try_fold(update, |node, key| node.get(key))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    if let Some(name) = claude {
        return Some(canonical(name, update));
    }
    if let Some(name) = mcp_name(update) {
        return Some(name);
    }
    if is_opening(update) || !known {
        if let Some(title) = update.get("title").and_then(Value::as_str) {
            if let Some(name) = title_tool_name(title, is_opening(update)) {
                if !is_its_command(update, name) {
                    return Some(canonical(name, update));
                }
            }
        }
    }
    if known {
        return None;
    }
    Kind::of(update).map(|kind| kind.tool_name(update).to_string())
}

fn mcp_name(update: &Value) -> Option<String> {
    if let Some(mcp) = update.pointer("/_meta/mcp") {
        let server = mcp.get("server")?.as_str()?;
        let tool = mcp.get("tool")?.as_str()?;
        if !server.is_empty() && !tool.is_empty() {
            return Some(format!("mcp__{server}__{tool}"));
        }
    }
    if update
        .pointer("/_meta/is_mcp_tool_call")
        .and_then(Value::as_bool)
        == Some(true)
    {
        if let Some(input) = update.get("rawInput") {
            if let (Some(server), Some(tool)) = (
                input.get("server").and_then(Value::as_str),
                input.get("tool").and_then(Value::as_str),
            ) {
                return Some(format!("mcp__{server}__{tool}"));
            }
        }
    }
    update
        .get("title")
        .and_then(Value::as_str)
        .and_then(parse_mcp_title)
        .map(|(server, tool)| format!("mcp__{server}__{tool}"))
}

fn parse_mcp_title(title: &str) -> Option<(&str, &str)> {
    let (prefix, rest) = title.split_once(" (")?;
    if !rest.ends_with(')') || (!rest.starts_with("Running ") && !rest.starts_with("Run ")) {
        return None;
    }
    let (server, tool) = prefix.split_once(": ")?;
    (is_tool_name(server) && is_tool_name(tool)).then_some((server, tool))
}

fn title_tool_name(title: &str, allow_bare: bool) -> Option<&str> {
    if allow_bare && is_tool_name(title) {
        return Some(title);
    }
    let stripped = title
        .strip_prefix("Running ")
        .or_else(|| title.strip_prefix("Run ").and_then(|s| s.strip_suffix('?')))?;
    is_prefixed_tool_name(stripped).then_some(stripped)
}

fn is_prefixed_tool_name(name: &str) -> bool {
    if !is_tool_name(name) {
        return false;
    }
    name.contains(['_', '-', '.'])
        || matches!(
            name,
            "execute"
                | "bash"
                | "shell"
                | "glob"
                | "grep"
                | "read"
                | "write"
                | "edit"
                | "ls"
                | "wait"
                | "schedule"
                | "task"
                | "webfetch"
                | "websearch"
                | "todowrite"
                | "todoread"
                | "skill"
        )
}

pub(super) fn is_generated_title(title: &str, named: Option<&str>) -> bool {
    named == Some(title)
        || title_tool_name(title, false).is_some()
        || parse_mcp_title(title).is_some()
        || (title.starts_with("Run `") && title.ends_with("`?"))
}

pub(super) fn parsed_raw_input(update: &Value) -> Option<Value> {
    let raw = update.get("rawInput").filter(|input| !input.is_null())?;
    if let Some(text) = raw.as_str() {
        if let Ok(parsed @ Value::Object(_)) = serde_json::from_str::<Value>(text) {
            return Some(parsed);
        }
    }
    Some(raw.clone())
}

fn is_its_command(update: &Value, title: &str) -> bool {
    let Some(input) = update.get("rawInput") else {
        return false;
    };
    ["command", "CommandLine", "command_line", "cmd"]
        .iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str))
        == Some(title)
}

fn is_opening(update: &Value) -> bool {
    update.get("sessionUpdate").and_then(Value::as_str) == Some(OPENING_FRAME)
}

fn is_tool_name(title: &str) -> bool {
    !title.is_empty()
        && title.len() <= MAX_NAME_LEN
        && title
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn canonical(name: &str, update: &Value) -> String {
    match name {
        "Agent" | "Task" => "task".to_string(),
        "execute" if has_code_input(update) => "code_execute".to_string(),
        other => other.to_string(),
    }
}

fn has_code_input(update: &Value) -> bool {
    let Some(raw) = parsed_raw_input(update) else {
        return false;
    };
    raw.get("code").is_some()
        || raw
            .get("arguments")
            .and_then(|args| args.get("code"))
            .is_some()
}
