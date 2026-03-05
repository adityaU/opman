//! Tool call formatting helpers and structured message parsing for Slack relay.

use anyhow::{Context, Result};

// ── Tool Call Formatting Helpers ─────────────────────────────────────────

/// Format a V2 tool part (`type: "tool"`) into a compact text representation.
///
/// V2 structure:
/// ```json
/// { "type": "tool", "tool": "bash", "callID": "...",
///   "state": { "status": "completed", "input": {...}, "output": "...", "title": "..." } }
/// ```
pub(crate) fn format_tool_part_v2(p: &serde_json::Value) -> String {
    let tool_name = p.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
    let state = match p.get("state") {
        Some(s) => s,
        None => return format!("`{}`", tool_name),
    };
    let status = state
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    match status {
        "completed" => {
            let title = state.get("title").and_then(|t| t.as_str()).unwrap_or("");
            if title.is_empty() {
                format!("`{}` (done)", tool_name)
            } else {
                format!("`{}`: {}", tool_name, title)
            }
        }
        "running" => {
            let title = state.get("title").and_then(|t| t.as_str()).unwrap_or("");
            if title.is_empty() {
                format!("`{}` (running)", tool_name)
            } else {
                format!("`{}`: {} (running)", tool_name, title)
            }
        }
        "error" => {
            let err = state
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            // Truncate long error messages to keep Slack output readable.
            let mut end = 200.min(err.len());
            while !err.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            let short_err = &err[..end];
            format!("`{}` error: {}", tool_name, short_err)
        }
        "pending" => format!("`{}` (pending)", tool_name),
        _ => format!("`{}` ({})", tool_name, status),
    }
}

/// Format a V1 tool-invocation part (`type: "tool-invocation"`) into text.
///
/// V1 structure:
/// ```json
/// { "type": "tool-invocation",
///   "toolInvocation": { "state": "result", "toolName": "bash", "args": {...}, "result": "..." } }
/// ```
pub(crate) fn format_tool_part_v1(p: &serde_json::Value) -> String {
    let inv = match p.get("toolInvocation") {
        Some(i) => i,
        None => return "`tool` (unknown)".to_string(),
    };
    let tool_name = inv
        .get("toolName")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");
    let state = inv
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    match state {
        "result" => format!("`{}` (done)", tool_name),
        "call" | "partial-call" => format!("`{}` (calling)", tool_name),
        _ => format!("`{}` ({})", tool_name, state),
    }
}

// ── Structured Tool Data ────────────────────────────────────────────────

/// A structured tool invocation extracted from an OpenCode session message.
/// Used to build Slack `task_update` streaming chunks.
#[derive(Debug, Clone)]
pub struct ToolPart {
    /// Tool name (e.g. "bash", "read", "edit").
    pub tool: String,
    /// Unique call ID for this invocation.
    pub call_id: String,
    /// Status: "pending", "running", "completed", "error".
    pub status: String,
    /// Human-readable title/summary of what the tool is doing.
    pub title: String,
    /// Optional output/result text (truncated for display).
    pub output: String,
}

impl ToolPart {
    /// Map OpenCode tool status to Slack `task_update` status values.
    /// Slack accepts: "pending", "in_progress", "complete", "error".
    pub fn slack_status(&self) -> &str {
        match self.status.as_str() {
            "completed" => "complete",
            "running" => "in_progress",
            "pending" => "pending",
            "error" => "error",
            // V1 states
            "result" => "complete",
            "call" | "partial-call" => "in_progress",
            _ => "in_progress",
        }
    }

    /// Convert this tool part into a Slack `task_update` chunk JSON value.
    pub fn to_task_chunk(&self) -> serde_json::Value {
        let mut chunk = serde_json::json!({
            "type": "task_update",
            "id": self.call_id,
            "title": if self.title.is_empty() {
                format!("`{}`", self.tool)
            } else {
                format!("`{}`: {}", self.tool, self.title)
            },
            "status": self.slack_status(),
        });
        if !self.output.is_empty() {
            // Strip XML-like tags and truncate output for Slack display.
            let cleaned = strip_xml_tags(&self.output);
            if !cleaned.is_empty() {
                // Use a larger limit for "task" (subagent) tool calls so that
                // the full subagent response is visible inside the task card.
                let max_len: usize = if self.tool == "task" { 4000 } else { 500 };
                let truncated = if cleaned.len() > max_len {
                    let mut end = max_len;
                    while !cleaned.is_char_boundary(end) && end > 0 {
                        end -= 1;
                    }
                    format!("{}\u{2026}", &cleaned[..end])
                } else {
                    cleaned
                };
                chunk["output"] = serde_json::Value::String(truncated.clone());
                // For error status, also populate `details` since Slack may
                // not render `output` for errored tasks in timeline view.
                if self.status == "error" {
                    chunk["details"] = serde_json::Value::String(truncated);
                }
            }
        }
        chunk
    }
}

/// Strip XML-like tags from tool output for cleaner Slack display.
/// Converts `<tag>content</tag>` to just `content` and removes self-closing tags.
fn strip_xml_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Clean up extra blank lines left behind by removed tags.
    let cleaned: Vec<&str> = result
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty())
        .collect();
    cleaned.join("\n")
}

/// Extract a `ToolPart` from a V2 tool part JSON value.
pub(crate) fn extract_tool_part_v2(p: &serde_json::Value) -> Option<ToolPart> {
    let tool = p.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
    let call_id = p
        .get("callID")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let state = p.get("state")?;
    let status = state
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown")
        .to_string();
    let title = state
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    // For error status, the message is in `state.error` rather than `state.output`.
    // Handle both string and object forms of the error field.
    let output = if status == "error" {
        let err_val = state.get("error");
        match err_val {
            Some(v) if v.is_string() => v.as_str().unwrap_or("").to_string(),
            Some(v) if v.is_object() => {
                // Try common sub-fields: "message", "text", then fall back to JSON.
                v.get("message")
                    .or_else(|| v.get("text"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string())
            }
            Some(v) => v.to_string(),
            None => {
                // Fallback: try `state.output` even for errors.
                state
                    .get("output")
                    .and_then(|o| o.as_str())
                    .unwrap_or("")
                    .to_string()
            }
        }
    } else {
        state
            .get("output")
            .and_then(|o| o.as_str())
            .unwrap_or("")
            .to_string()
    };

    Some(ToolPart {
        tool: tool.to_string(),
        call_id: if call_id.is_empty() {
            format!("tool_{}_{}", tool, status)
        } else {
            call_id
        },
        status,
        title,
        output,
    })
}

/// Extract a `ToolPart` from a V1 tool-invocation part JSON value.
pub(crate) fn extract_tool_part_v1(p: &serde_json::Value) -> Option<ToolPart> {
    let inv = p.get("toolInvocation")?;
    let tool = inv
        .get("toolName")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");
    let state = inv
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let tool_call_id = inv.get("toolCallId").and_then(|c| c.as_str()).unwrap_or("");
    let result = inv.get("result").and_then(|r| r.as_str()).unwrap_or("");

    Some(ToolPart {
        tool: tool.to_string(),
        call_id: if tool_call_id.is_empty() {
            format!("v1_{}_{}", tool, state)
        } else {
            tool_call_id.to_string()
        },
        status: state.to_string(),
        title: String::new(),
        output: result.to_string(),
    })
}

/// A session message with both rendered text and structured tool data.
#[derive(Debug, Clone)]
pub struct StructuredMessage {
    pub role: String,
    /// Text content (from text parts, with tool parts formatted as text).
    pub text: String,
    /// Structured tool invocations found in this message.
    pub tools: Vec<ToolPart>,
}

/// Fetch messages for a session, returning structured data including tool parts.
/// This is the rich version of `fetch_all_session_messages` that preserves tool
/// metadata for use with Slack `task_update` streaming chunks.
pub async fn fetch_session_messages_with_tools(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
) -> Result<Vec<StructuredMessage>> {
    let url = format!("{}/session/{}/message", base_url, session_id);

    let response = client
        .get(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to fetch session messages")?;

    let body: serde_json::Value = response.json().await?;

    let mut messages = Vec::new();
    let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.iter().collect()
    } else if let Some(obj) = body.as_object() {
        obj.values().collect()
    } else {
        vec![]
    };

    for item in items {
        let info = item.get("info");
        let role = info
            .and_then(|i| i.get("role"))
            .and_then(|r| r.as_str())
            .or_else(|| item.get("role").and_then(|r| r.as_str()))
            .unwrap_or("")
            .to_string();

        let mut text_parts = Vec::new();
        let mut tool_parts = Vec::new();

        if let Some(parts) = item.get("parts").and_then(|p| p.as_array()) {
            for p in parts {
                let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match ptype {
                    "text" | "" => {
                        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                            if !t.is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                    }
                    "tool" => {
                        // Extract structured tool data for task_update chunks.
                        // Only fall back to formatted text if structured
                        // extraction fails, to avoid duplicate display.
                        if let Some(tp) = extract_tool_part_v2(p) {
                            tool_parts.push(tp);
                        } else {
                            let formatted = format_tool_part_v2(p);
                            if !formatted.is_empty() {
                                text_parts.push(formatted);
                            }
                        }
                    }
                    "tool-invocation" => {
                        if let Some(tp) = extract_tool_part_v1(p) {
                            tool_parts.push(tp);
                        } else {
                            let formatted = format_tool_part_v1(p);
                            if !formatted.is_empty() {
                                text_parts.push(formatted);
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
            text_parts.push(t.to_string());
        } else if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
            text_parts.push(content.to_string());
        }

        let text = text_parts.join("\n");
        if !text.is_empty() || !tool_parts.is_empty() {
            messages.push(StructuredMessage {
                role,
                text,
                tools: tool_parts,
            });
        }
    }

    Ok(messages)
}

/// Build an array of Slack `task_update` chunk JSON values from tool parts.
pub fn build_task_chunks(tools: &[ToolPart]) -> Vec<serde_json::Value> {
    tools.iter().map(|t| t.to_task_chunk()).collect()
}
