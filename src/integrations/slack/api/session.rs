//! Session-related Slack API helpers: fetching messages, sending user/system
//! messages, and finding free sessions.

use anyhow::{Context, Result};
use tracing::debug;

// ── Fetch Assistant Messages ────────────────────────────────────────────

/// Fetch messages for a session, including assistant messages.
/// Used by the response batcher to collect AI responses for Slack relay.
pub async fn fetch_all_session_messages(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
) -> Result<Vec<(String, String)>> {
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

        let text = if let Some(parts) = item.get("parts").and_then(|p| p.as_array()) {
            parts
                .iter()
                .filter_map(|p| {
                    let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match ptype {
                        "text" | "" => p
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string()),
                        "tool" => {
                            Some(crate::integrations::slack::tools::format_tool_part_v2(p))
                        }
                        "tool-invocation" => {
                            Some(crate::integrations::slack::tools::format_tool_part_v1(p))
                        }
                        _ => None,
                    }
                })
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
            t.to_string()
        } else if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
            content.to_string()
        } else {
            continue;
        };

        if !text.is_empty() {
            messages.push((role, text));
        }
    }

    Ok(messages)
}

// ── Send User Message (non-system) ──────────────────────────────────────

/// Send a user message to a session asynchronously via the OpenCode API.
/// Uses `POST /session/{id}/prompt_async` with `system: "false"`.
pub async fn send_user_message(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
    text: &str,
) -> Result<()> {
    let url = format!("{}/session/{}/prompt_async", base_url, session_id);
    debug!(url, session_id, "Sending user message to session via Slack");

    let resp = client
        .post(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "system": "false",
            "parts": [{ "type": "text", "text": text }]
        }))
        .send()
        .await
        .context("Failed to send user message to opencode session")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "User message rejected by server: HTTP {} — {}",
            status,
            body
        );
    }

    Ok(())
}

/// Send a system message to a session (for thread replies).
pub async fn send_system_message(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
    text: &str,
) -> Result<()> {
    let url = format!("{}/session/{}/prompt_async", base_url, session_id);
    debug!(
        url,
        session_id, "Sending system message to session via Slack thread reply"
    );

    let resp = client
        .post(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "system": "true",
            "parts": [{ "type": "text", "text": text }]
        }))
        .send()
        .await
        .context("Failed to send system message to opencode session")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "System message rejected by server: HTTP {} — {}",
            status,
            body
        );
    }

    Ok(())
}

/// Find a "free" session in the given project — one that is not currently
/// actively processing a request.  Returns the **most recently used** session
/// among idle candidates (preferring the one whose context is freshest).
///
/// A session is considered free if:
/// - It is NOT a subagent session (no parent_id)
/// - It is NOT in `active_sessions` (not currently busy)
///
/// The `idle_minutes` parameter is ignored — any non-busy session qualifies.
pub fn find_free_session(
    sessions: &[crate::app::SessionInfo],
    active_sessions: &std::collections::HashSet<String>,
    _idle_minutes: u64,
) -> Option<String> {
    let mut candidates: Vec<&crate::app::SessionInfo> = sessions
        .iter()
        .filter(|s| {
            // Skip subagent sessions (those with a parent).
            if !s.parent_id.is_empty() {
                debug!(
                    "  session {} — skipped (subagent)",
                    &s.id[..8.min(s.id.len())]
                );
                return false;
            }
            // Skip sessions that are currently active/busy.
            if active_sessions.contains(&s.id) {
                debug!(
                    "  session {} — skipped (active/busy)",
                    &s.id[..8.min(s.id.len())]
                );
                return false;
            }
            debug!(
                "  session {} — eligible (not busy, updated={})",
                &s.id[..8.min(s.id.len())],
                s.time.updated,
            );
            true
        })
        .collect();

    debug!(
        "find_free_session: {} candidate(s) out of {} total sessions",
        candidates.len(),
        sessions.len()
    );

    // Sort by updated time descending — most recently used (highest timestamp) first.
    // This prefers sessions with fresher context.
    candidates.sort_by(|a, b| b.time.updated.cmp(&a.time.updated));

    let result = candidates.first().map(|s| s.id.clone());
    if let Some(ref sid) = result {
        debug!(
            "find_free_session: selected {} (most recently used)",
            &sid[..8.min(sid.len())]
        );
    } else {
        debug!("find_free_session: no free session found");
    }
    result
}
