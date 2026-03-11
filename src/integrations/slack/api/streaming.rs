//! Streaming Slack messages (chat.startStream, chat.appendStream, chat.stopStream)
//! and text chunking utilities.

use anyhow::{Context, Result};

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
