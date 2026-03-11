//! Interactive and slash command payload handling for Slack Socket Mode.

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::integrations::slack::types::SlackBackgroundEvent;

/// Parse a Slack interactive (block_actions) payload and emit a BlockAction event.
///
/// The payload shape for `block_actions` is:
/// ```json
/// {
///   "type": "block_actions",
///   "actions": [{ "action_id": "...", "block_id": "...", ... }],
///   "channel": { "id": "C..." },
///   "message": { "ts": "..." },
///   "container": { "thread_ts": "..." },
///   "user": { "id": "U..." }
/// }
/// ```
pub(super) fn handle_interactive_payload(
    payload: &serde_json::Value,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
) -> Result<()> {
    let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match payload_type {
        "block_actions" => handle_block_actions_payload(payload, event_tx),
        "view_submission" => handle_view_submission_payload(payload, event_tx),
        _ => {
            debug!("Ignoring interactive payload type: {}", payload_type);
            Ok(())
        }
    }
}

/// Parse a block_actions interactive payload and emit a BlockAction event.
fn handle_block_actions_payload(
    payload: &serde_json::Value,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
) -> Result<()> {
    let actions = payload
        .get("actions")
        .and_then(|v| v.as_array())
        .context("Missing 'actions' array in block_actions payload")?;

    let channel = payload
        .pointer("/channel/id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let message_ts = payload
        .pointer("/message/ts")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // thread_ts may be in container or message.
    let thread_ts = payload
        .pointer("/container/thread_ts")
        .or_else(|| payload.pointer("/message/thread_ts"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let user = payload
        .pointer("/user/id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    for action in actions {
        let action_id = action
            .get("action_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if action_id.is_empty() {
            continue;
        }

        info!(
            "Slack block_action: action_id={}, channel={}, message_ts={}, thread_ts={:?}, user={}",
            action_id, channel, message_ts, thread_ts, user
        );

        let _ = event_tx.send(SlackBackgroundEvent::BlockAction {
            action_id: action_id.to_string(),
            channel: channel.to_string(),
            message_ts: message_ts.to_string(),
            thread_ts: thread_ts.clone(),
            user: user.to_string(),
        });
    }

    Ok(())
}

/// Parse a view_submission interactive payload and emit a ViewSubmission event.
///
/// The payload shape for `view_submission` is:
/// ```json
/// {
///   "type": "view_submission",
///   "user": { "id": "U..." },
///   "view": {
///     "callback_id": "sessions_modal",
///     "private_metadata": "...",
///     "state": {
///       "values": {
///         "block_id": {
///           "action_id": { "type": "static_select", "selected_option": { "value": "..." } }
///         }
///       }
///     }
///   }
/// }
/// ```
fn handle_view_submission_payload(
    payload: &serde_json::Value,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
) -> Result<()> {
    let callback_id = payload
        .pointer("/view/callback_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let user = payload
        .pointer("/user/id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let values = payload
        .pointer("/view/state/values")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let private_metadata = payload
        .pointer("/view/private_metadata")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let trigger_id = payload
        .get("trigger_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if callback_id.is_empty() {
        debug!("Ignoring view_submission with empty callback_id");
        return Ok(());
    }

    info!(
        "Slack view_submission: callback_id={}, user={}, private_metadata={}, trigger_id={}",
        callback_id, user, private_metadata, trigger_id
    );

    let _ = event_tx.send(SlackBackgroundEvent::ViewSubmission {
        callback_id: callback_id.to_string(),
        user: user.to_string(),
        values,
        private_metadata: private_metadata.to_string(),
        trigger_id: trigger_id.to_string(),
    });

    Ok(())
}

/// Parse a Slack slash command payload and emit a SlashCommand event.
///
/// Socket Mode delivers slash commands as:
/// ```json
/// {
///   "command": "/opman-projects",
///   "text": "optional args",
///   "user_id": "U...",
///   "channel_id": "D...",
///   "response_url": "https://hooks.slack.com/...",
///   "trigger_id": "..."
/// }
/// ```
pub(super) fn handle_slash_command_payload(
    payload: &serde_json::Value,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
) -> Result<()> {
    let command = payload
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let text = payload.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let channel = payload
        .get("channel_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let user = payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let response_url = payload
        .get("response_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let trigger_id = payload
        .get("trigger_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if command.is_empty() {
        debug!("Ignoring slash command payload with empty command");
        return Ok(());
    }

    info!(
        "Slack slash command: command={}, text={}, channel={}, user={}",
        command, text, channel, user
    );

    let _ = event_tx.send(SlackBackgroundEvent::SlashCommand {
        command: command.to_string(),
        text: text.to_string(),
        channel: channel.to_string(),
        user: user.to_string(),
        response_url: response_url.to_string(),
        trigger_id: trigger_id.to_string(),
    });

    Ok(())
}
