//! Slack modals and response-URL helpers (views.open, response_url POST).

use anyhow::{Context, Result};

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
