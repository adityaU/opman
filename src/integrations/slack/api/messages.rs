//! Post and update Slack messages (chat.postMessage, chat.update).

use anyhow::{Context, Result};

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
