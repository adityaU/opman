//! Relay content builder – processes new session messages into relay text,
//! raw text, task chunks, and role groups.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::super::formatting::convert_markdown_tables;
use super::super::state::{slack_thread_link, SlackState};
use super::super::tools::{build_task_chunks, StructuredMessage};

/// Process new messages into relay text, raw text, task chunks, and role
/// groups.  Returns `(relay_text, relay_text_raw, all_task_chunks, groups)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn build_relay_content(
    new_messages: &[StructuredMessage],
    session_id: &str,
    last_streamed_role: &Option<String>,
    label: &Option<String>,
    label_emitted: bool,
    slack_state: &Arc<Mutex<SlackState>>,
) -> (
    String,
    String,
    Vec<serde_json::Value>,
    Vec<(String, Vec<String>)>,
) {
    let mut all_task_chunks: Vec<serde_json::Value> = Vec::new();
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut groups_raw: Vec<(String, Vec<String>)> = Vec::new();

    let subagent_links: Vec<String> = {
        let s = slack_state.lock().await;
        s.subagent_threads
            .iter()
            .filter(|(_child_sid, (_ch, _ts, parent_sid))| parent_sid == session_id)
            .map(|(_child_sid, (ch, ts, _parent_sid))| slack_thread_link(ch, ts))
            .collect()
    };

    for msg in new_messages {
        if !msg.tools.is_empty() {
            let mut chunks = build_task_chunks(&msg.tools);
            if !subagent_links.is_empty() {
                let mut link_idx = 0;
                for chunk in &mut chunks {
                    let is_task = chunk
                        .get("title")
                        .and_then(|t| t.as_str())
                        .map(|t| t.starts_with("`task`:"))
                        .unwrap_or(false);
                    if is_task {
                        let link = if link_idx < subagent_links.len() {
                            &subagent_links[link_idx]
                        } else {
                            subagent_links.last().unwrap()
                        };
                        chunk["output"] = serde_json::Value::String(format!(
                            ":thread: Subagent thread: {}",
                            link
                        ));
                        link_idx += 1;
                    }
                }
            }
            all_task_chunks.extend(chunks);
        }

        if !msg.text.is_empty() {
            let converted = convert_markdown_tables(&msg.text);
            let raw = msg.text.clone();
            if let Some(last) = groups.last_mut() {
                if last.0 == msg.role {
                    last.1.push(converted);
                    if let Some(last_raw) = groups_raw.last_mut() {
                        last_raw.1.push(raw);
                    }
                    continue;
                }
            }
            groups.push((msg.role.clone(), vec![converted]));
            groups_raw.push((msg.role.clone(), vec![raw]));
        }
    }

    let relay_text = format_groups(&groups, last_streamed_role);

    let relay_text_raw = {
        let mut raw = format_groups(&groups_raw, last_streamed_role);
        if !label_emitted {
            if let Some(ref lbl) = label {
                raw = format!("{}\n{}", lbl, raw);
            }
        }
        raw
    };

    // Diagnostic log.
    if relay_text != relay_text_raw {
        tracing::info!(
            "Slack relay: relay_text DIFFERS from relay_text_raw — converted_len={}, raw_len={}, raw_preview={:?}",
            relay_text.len(),
            relay_text_raw.len(),
            crate::util::truncate_str(&relay_text_raw, 400)
        );
    } else {
        tracing::info!(
            "Slack relay: relay_text SAME as relay_text_raw — len={}",
            relay_text.len()
        );
    }

    (relay_text, relay_text_raw, all_task_chunks, groups)
}

/// Format role-grouped messages into markdown relay text.
fn format_groups(
    groups: &[(String, Vec<String>)],
    last_streamed_role: &Option<String>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, (role, texts)) in groups.iter().enumerate() {
        let body = texts.join("\n");
        let same_as_last = i == 0 && last_streamed_role.as_deref() == Some(role.as_str());

        let formatted = if role == "user" {
            body.lines()
                .map(|l| format!("> {}", l))
                .collect::<Vec<_>>()
                .join("\n")
        } else if role == "error" {
            format!(":x: {}", body)
        } else {
            body
        };

        if same_as_last {
            parts.push(formatted);
        } else {
            parts.push(format!("\n{}", formatted));
        }
    }
    parts.join("\n")
}
