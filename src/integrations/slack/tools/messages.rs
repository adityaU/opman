//! Session message fetching with structured tool data extraction.

use anyhow::{Context, Result};

use super::{
    extract_tool_part_v1, extract_tool_part_v2, format_tool_part_v1, format_tool_part_v2,
    StructuredMessage, ToolPart,
};

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
        let mut tool_parts: Vec<ToolPart> = Vec::new();

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
