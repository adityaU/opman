//! Slack Web API helper functions (HTTP wrappers for chat.postMessage, etc.).

use anyhow::{Context, Result};
use tracing::debug;

// ── Slack Web API Helpers ───────────────────────────────────────────────

/// Post a message to a Slack channel/thread.
pub async fn post_message(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    text: &str,
    thread_ts: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "channel": channel,
        "text": text,
    });
    if let Some(ts) = thread_ts {
        body["thread_ts"] = serde_json::Value::String(ts.to_string());
    }

    let resp = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to post Slack message")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.postMessage failed: {}", err);
    }

    result
        .get("ts")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'ts' in postMessage response")
}

/// Post a message with Block Kit blocks to a Slack channel/thread.
///
/// `text` serves as the notification/fallback text.
/// `blocks` is the Block Kit layout array.
/// `attachments` is an optional array of attachment objects (used for table blocks).
pub async fn post_message_with_blocks(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    text: &str,
    blocks: &[serde_json::Value],
    attachments: Option<&[serde_json::Value]>,
    thread_ts: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "channel": channel,
        "text": text,
        "blocks": blocks,
    });
    if let Some(ts) = thread_ts {
        body["thread_ts"] = serde_json::Value::String(ts.to_string());
    }
    if let Some(att) = attachments {
        if !att.is_empty() {
            body["attachments"] = serde_json::json!(att);
        }
    }

    let resp = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to post Slack message with blocks")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.postMessage (blocks) failed: {}", err);
    }

    result
        .get("ts")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'ts' in postMessage response")
}

/// Update (edit) an existing Slack message.
#[allow(dead_code)]
pub async fn update_message(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
    text: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "channel": channel,
        "ts": ts,
        "text": text,
    });

    let resp = client
        .post("https://slack.com/api/chat.update")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to update Slack message")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.update failed: {}", err);
    }

    Ok(())
}

/// Update (edit) an existing Slack message, replacing its blocks.
///
/// Sends `text` as fallback and `blocks` as the new Block Kit layout.
/// `attachments` is an optional array of attachment objects (used for table blocks).
/// Pass an empty slice for `blocks` to remove all blocks (e.g. after a button is clicked).
pub async fn update_message_blocks(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
    text: &str,
    blocks: &[serde_json::Value],
    attachments: Option<&[serde_json::Value]>,
) -> Result<()> {
    let mut body = serde_json::json!({
        "channel": channel,
        "ts": ts,
        "text": text,
        "blocks": blocks,
    });
    if let Some(att) = attachments {
        if !att.is_empty() {
            body["attachments"] = serde_json::json!(att);
        }
    }

    let resp = client
        .post("https://slack.com/api/chat.update")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to update Slack message with blocks")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.update (blocks) failed: {}", err);
    }

    Ok(())
}

// ── Streaming API ───────────────────────────────────────────────────────

/// Start a new streaming message in a Slack thread.
/// Returns the stream message `ts` on success.
///
/// `task_display_mode` can be `"timeline"` (default, sequential task cards)
/// or `"plan"` (grouped task display).
pub async fn start_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    markdown_text: Option<&str>,
    chunks: Option<&[serde_json::Value]>,
    task_display_mode: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "channel": channel,
        "thread_ts": thread_ts,
    });

    // Slack does not allow both top-level `markdown_text` AND `chunks` in the
    // same request.  When we have chunks, embed the text as a leading
    // `markdown_text` chunk inside the chunks array instead.
    match (markdown_text, chunks) {
        (Some(text), Some(c)) => {
            let mut all_chunks: Vec<serde_json::Value> = Vec::with_capacity(c.len() + 1);
            all_chunks.push(serde_json::json!({
                "type": "markdown_text",
                "text": text,
            }));
            all_chunks.extend(c.iter().cloned());
            body["chunks"] = serde_json::Value::Array(all_chunks);
        }
        (Some(text), None) => {
            body["markdown_text"] = serde_json::Value::String(text.to_string());
        }
        (None, Some(c)) => {
            body["chunks"] = serde_json::Value::Array(c.to_vec());
        }
        (None, None) => {}
    }
    if let Some(mode) = task_display_mode {
        body["task_display_mode"] = serde_json::Value::String(mode.to_string());
    }

    let resp = client
        .post("https://slack.com/api/chat.startStream")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to start Slack stream")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.startStream failed: {}", err);
    }

    result
        .get("ts")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'ts' in startStream response")
}

/// Append content to an active streaming message.
pub async fn append_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
    markdown_text: &str,
    chunks: Option<&[serde_json::Value]>,
) -> Result<()> {
    let mut body = serde_json::json!({
        "channel": channel,
        "ts": ts,
    });

    // Slack does not allow both top-level `markdown_text` AND `chunks`.
    // When chunks are present, embed the text as a `markdown_text` chunk.
    if let Some(c) = chunks {
        let mut all_chunks: Vec<serde_json::Value> = Vec::with_capacity(c.len() + 1);
        if !markdown_text.is_empty() {
            all_chunks.push(serde_json::json!({
                "type": "markdown_text",
                "text": markdown_text,
            }));
        }
        all_chunks.extend(c.iter().cloned());
        body["chunks"] = serde_json::Value::Array(all_chunks);
    } else {
        body["markdown_text"] = serde_json::Value::String(markdown_text.to_string());
    }

    let resp = client
        .post("https://slack.com/api/chat.appendStream")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to append to Slack stream")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.appendStream failed: {}", err);
    }

    Ok(())
}

/// Stop (finalize) an active streaming message.
pub async fn stop_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "channel": channel,
        "ts": ts,
    });

    let resp = client
        .post("https://slack.com/api/chat.stopStream")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to stop Slack stream")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.stopStream failed: {}", err);
    }

    Ok(())
}

/// Split text into chunks that fit within Slack's 40,000 character limit.
/// Tries to split on newline boundaries for cleanliness.
pub fn chunk_for_slack(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to find a newline near the limit to split cleanly.
        let search_start = max_chars.saturating_sub(200);
        let split_at = remaining[search_start..max_chars]
            .rfind('\n')
            .map(|i| search_start + i + 1)
            .unwrap_or(max_chars);

        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }

    chunks
}

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
                        "tool" => Some(super::tools::format_tool_part_v2(p)),
                        "tool-invocation" => Some(super::tools::format_tool_part_v1(p)),
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

/// Post a message to a Slack slash command `response_url`.
///
/// The response_url accepts a JSON payload with `text` and optional
/// `response_type` ("ephemeral" for only-visible-to-user, "in_channel" for
/// visible to everyone). Defaults to "ephemeral".
///
/// This does NOT require a bot token — the response_url is pre-authenticated.
pub async fn post_to_response_url(
    client: &reqwest::Client,
    response_url: &str,
    text: &str,
    ephemeral: bool,
) -> Result<()> {
    let response_type = if ephemeral { "ephemeral" } else { "in_channel" };
    let body = serde_json::json!({
        "text": text,
        "response_type": response_type,
    });

    let resp = client
        .post(response_url)
        .json(&body)
        .send()
        .await
        .context("Failed to post to response_url")?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("response_url POST failed (HTTP {}): {}", status, body_text);
    }

    Ok(())
}

/// Open a Slack modal (dialog) using a trigger_id and a view payload.
///
/// Calls `POST https://slack.com/api/views.open` with the given trigger_id
/// and view JSON.  The trigger_id must be used within 3 seconds of receiving it.
///
/// Returns the view ID on success (useful for later `views.update` calls).
pub async fn open_modal(
    client: &reqwest::Client,
    bot_token: &str,
    trigger_id: &str,
    view: &serde_json::Value,
) -> Result<String> {
    let body = serde_json::json!({
        "trigger_id": trigger_id,
        "view": view,
    });

    let resp = client
        .post("https://slack.com/api/views.open")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to call views.open")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("views.open failed: {}", err);
    }

    result
        .pointer("/view/id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'view.id' in views.open response")
}
