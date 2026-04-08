//! Session transcript formatter — produces Markdown from messages.
//! Used by the `/copy` command to copy the full session to clipboard.

use crate::types::core::Message;

/// Format a session transcript as Markdown.
///
/// Produces output matching the OpenCode TUI `/copy` format:
/// - Session header with title, ID, timestamps
/// - Each message with role label, metadata, and content
/// - Tool calls included with name and status
pub fn format_transcript(title: &str, session_id: &str, messages: &[Message]) -> String {
    let mut out = String::with_capacity(4096);

    // Header
    out.push_str("# ");
    out.push_str(if title.is_empty() {
        "Untitled Session"
    } else {
        title
    });
    out.push_str("\n\n");
    out.push_str("**Session ID:** ");
    out.push_str(session_id);
    out.push_str("\n\n---\n\n");

    for msg in messages {
        format_message(&mut out, msg);
        out.push_str("\n---\n\n");
    }

    out
}

fn format_message(out: &mut String, msg: &Message) {
    let role = &msg.info.role;
    let label = match role.as_str() {
        "user" => "You",
        "assistant" => "Assistant",
        _ => role.as_str(),
    };

    // Role header
    out.push_str("## ");
    out.push_str(label);

    // Model metadata for assistant messages
    if role == "assistant" {
        if let Some(ref model_id) = msg.info.model_id {
            out.push_str(" (");
            // Use short model name: last segment after '/'
            let short = model_id.rsplit('/').next().unwrap_or(model_id);
            out.push_str(short);
            out.push(')');
        }
        if let Some(ref agent) = msg.info.agent {
            if !agent.is_empty() {
                out.push_str(" [");
                out.push_str(agent);
                out.push(']');
            }
        }
    }
    out.push_str("\n\n");

    // Parts
    for part in &msg.parts {
        format_part(out, part);
    }
}

fn format_part(out: &mut String, part: &crate::types::core::MessagePart) {
    match part.part_type.as_str() {
        "text" => {
            if let Some(ref text) = part.text {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push_str(trimmed);
                    out.push_str("\n\n");
                }
            }
        }
        "tool" => {
            let tool_name = part
                .tool_name
                .as_deref()
                .or(part.tool.as_deref())
                .unwrap_or("unknown");
            out.push_str("**Tool: ");
            out.push_str(tool_name);
            out.push_str("**");

            // Status
            if let Some(ref state) = part.state {
                if let Some(ref status) = state.status {
                    out.push_str(" (");
                    out.push_str(status);
                    out.push(')');
                }
            }
            out.push('\n');

            // Tool input (args)
            if let Some(ref args) = part.args {
                if let Some(s) = args.as_str() {
                    if !s.is_empty() {
                        out.push_str("```\n");
                        out.push_str(s);
                        out.push_str("\n```\n");
                    }
                } else if !args.is_null() {
                    if let Ok(pretty) = serde_json::to_string_pretty(args) {
                        out.push_str("```json\n");
                        out.push_str(&pretty);
                        out.push_str("\n```\n");
                    }
                }
            }

            // Tool output
            if let Some(ref state) = part.state {
                if let Some(ref output) = state.output {
                    let trimmed = output.trim();
                    if !trimmed.is_empty() {
                        out.push_str("```\n");
                        // Cap output at 500 chars for readability
                        if trimmed.len() > 500 {
                            out.push_str(&trimmed[..500]);
                            out.push_str("\n... (truncated)");
                        } else {
                            out.push_str(trimmed);
                        }
                        out.push_str("\n```\n");
                    }
                }
                if let Some(ref error) = state.error {
                    let trimmed = error.trim();
                    if !trimmed.is_empty() {
                        out.push_str("**Error:** ");
                        out.push_str(trimmed);
                        out.push('\n');
                    }
                }
            }
            out.push('\n');
        }
        _ => {} // skip reasoning, synthetic parts, etc.
    }
}
