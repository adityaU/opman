//! Helper functions and types for tool call rendering.

use crate::types::core::{Message, MessagePart};

/// Shorten tool names by removing common prefixes (provider_provider_action -> action).
pub fn format_tool_name(name: &str) -> String {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() >= 3 && parts[0] == parts[1] {
        return parts[2..].join("_");
    }
    if parts.len() == 2 && parts[0] == parts[1] {
        return parts[0].to_string();
    }
    name.to_string()
}

/// Format milliseconds as a human-readable duration.
pub fn format_duration(ms: f64) -> String {
    if ms < 1000.0 {
        format!("{}ms", ms as u64)
    } else {
        let s = ms / 1000.0;
        if s < 60.0 {
            format!("{:.1}s", s)
        } else {
            let m = (s / 60.0).floor() as u64;
            let rem = s % 60.0;
            format!("{}m {}s", m, rem as u64)
        }
    }
}

/// Extract the child session ID for a task tool part.
pub fn get_task_session_id(
    part: &MessagePart,
    child_session: Option<&ChildSessionRef>,
) -> Option<String> {
    // Primary: state.metadata.sessionId
    if let Some(ref state) = part.state {
        if let Some(ref meta) = state.metadata {
            if let Some(sid) = meta.get("sessionId").and_then(|v| v.as_str()) {
                if !sid.is_empty() {
                    return Some(sid.to_string());
                }
            }
        }
        // Secondary: state.input.task_id
        if let Some(ref input) = state.input {
            if let Some(obj) = input.as_object() {
                if let Some(tid) = obj.get("task_id").and_then(|v| v.as_str()) {
                    if !tid.is_empty() {
                        return Some(tid.to_string());
                    }
                }
            }
        }
    }
    // Tertiary: positionally-matched session
    child_session.map(|cs| cs.id.clone())
}

/// Structured output types.
pub enum ParsedOutput {
    File { path: String, content: String },
    Markdown { content: String },
    Plain { content: String },
}

/// Parse structured output from tool results.
pub fn parse_output(output: &str) -> ParsedOutput {
    // Check for file content XML pattern: <path>...</path>...<content>...</content>
    if let Some(path_start) = output.find("<path>") {
        if let Some(path_end) = output.find("</path>") {
            if let Some(content_start) = output.find("<content>") {
                if let Some(content_end) = output.find("</content>") {
                    let path = &output[path_start + 6..path_end];
                    let content = &output[content_start + 9..content_end];
                    return ParsedOutput::File {
                        path: path.to_string(),
                        content: content.to_string(),
                    };
                }
            }
        }
    }

    // Check for task_result markdown pattern
    if let Some(start) = output.find("<task_result>") {
        if let Some(end) = output.find("</task_result>") {
            let content = &output[start + 13..end];
            return ParsedOutput::Markdown {
                content: content.trim().to_string(),
            };
        }
    }

    ParsedOutput::Plain {
        content: output.to_string(),
    }
}

/// A reference to a child session (for task tool matching).
#[derive(Debug, Clone)]
pub struct ChildSessionRef {
    pub id: String,
    pub title: String,
}

/// Subagent messages map type alias.
pub type SubagentMessagesMap = std::collections::HashMap<String, Vec<Message>>;

/// Compute auto-expand default for a tool call accordion.
pub fn auto_expand_default(
    is_todo_write: bool,
    is_task_tool: bool,
    is_bash_tool: bool,
    is_a2ui: bool,
    is_running: bool,
    is_completed: bool,
    is_error: bool,
    has_subagent_messages: bool,
) -> bool {
    // A2UI and todo_write are always expanded
    if is_a2ui || is_todo_write {
        return true;
    }
    let mut exp = is_task_tool && (is_running || is_completed || is_error);
    if is_bash_tool && is_running {
        exp = true;
    }
    if is_task_tool && (is_running || has_subagent_messages) {
        exp = true;
    }
    if is_completed {
        exp = false;
    }
    exp
}
