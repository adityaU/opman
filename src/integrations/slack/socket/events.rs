//! Events API payload handling for Slack Socket Mode.

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::integrations::slack::types::SlackBackgroundEvent;

/// Parse an Events API payload and emit the appropriate SlackBackgroundEvent.
pub(super) fn handle_events_api_payload(
    payload: &serde_json::Value,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
    our_user_id: &str,
) -> Result<()> {
    let event = payload
        .get("event")
        .context("Missing 'event' in events_api payload")?;

    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

    debug!(
        "Slack event: type={}, user={}, subtype={}, bot_id={}, channel_type={}",
        event_type,
        event
            .get("user")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
        event
            .get("subtype")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
        event
            .get("bot_id")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
        event
            .get("channel_type")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
    );

    if event_type != "message" {
        debug!("Ignoring non-message event type: {}", event_type);
        return Ok(());
    }

    // Ignore bot-generated messages (our own replies, other bots).
    // Only filter the specific "bot_message" subtype — normal user messages
    // in DMs can carry other subtypes (e.g. "file_share") that we want to accept.
    if event.get("bot_id").is_some() {
        debug!("Ignoring bot message (bot_id present)");
        return Ok(());
    }
    let subtype = event.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
    if subtype == "bot_message" || subtype == "bot_add" || subtype == "bot_remove" {
        debug!("Ignoring bot subtype: {}", subtype);
        return Ok(());
    }

    let user = event.get("user").and_then(|v| v.as_str()).unwrap_or("");
    // Only process messages from ourselves (self-chat DM).
    if user != our_user_id {
        debug!(
            "Ignoring message from other user: {} (expected {})",
            user, our_user_id
        );
        return Ok(());
    }

    let text = event.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let channel = event.get("channel").and_then(|v| v.as_str()).unwrap_or("");
    let ts = event.get("ts").and_then(|v| v.as_str()).unwrap_or("");
    let thread_ts = event.get("thread_ts").and_then(|v| v.as_str());

    if text.is_empty() || channel.is_empty() || ts.is_empty() {
        debug!(
            "Ignoring message with empty fields: text={}, channel={}, ts={}",
            !text.is_empty(),
            !channel.is_empty(),
            !ts.is_empty()
        );
        return Ok(());
    }

    if let Some(parent_ts) = thread_ts {
        // This is a reply in a thread.
        info!(
            "Slack incoming thread reply: channel={} ts={} thread_ts={}",
            channel, ts, parent_ts
        );
        let _ = event_tx.send(SlackBackgroundEvent::IncomingThreadReply {
            text: text.to_string(),
            channel: channel.to_string(),
            ts: ts.to_string(),
            thread_ts: parent_ts.to_string(),
            user: user.to_string(),
        });
    } else {
        // Top-level message — needs triage.
        info!(
            "Slack incoming message: channel={} ts={} text={}...",
            channel,
            ts,
            crate::util::truncate_str(&text, 50)
        );
        let _ = event_tx.send(SlackBackgroundEvent::IncomingMessage {
            text: text.to_string(),
            channel: channel.to_string(),
            ts: ts.to_string(),
            user: user.to_string(),
        });
    }

    Ok(())
}
