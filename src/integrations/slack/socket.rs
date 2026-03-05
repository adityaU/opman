//! Slack Socket Mode WebSocket connection and event handling.

use std::time::Duration;

use anyhow::{Context, Result};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::auth::SlackAuth;
use super::types::{SlackBackgroundEvent, SlackConnectionStatus};

// ── Socket Mode WebSocket Connection ────────────────────────────────────

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Request a WebSocket URL from Slack's apps.connections.open endpoint.
async fn get_ws_url(client: &reqwest::Client, app_token: &str) -> Result<String> {
    let resp = client
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(app_token)
        .send()
        .await
        .context("Failed to call apps.connections.open")?;

    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("apps.connections.open failed: {}", err);
    }

    body.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'url' in connections.open response")
}

/// Connect to Slack Socket Mode and process events.
/// Sends parsed events to the main loop via `event_tx`.
/// Reconnects automatically on disconnection.
pub async fn spawn_socket_mode(
    auth: SlackAuth,
    event_tx: mpsc::UnboundedSender<SlackBackgroundEvent>,
) {
    let client = reqwest::Client::new();
    let our_user_id = auth.user_id.clone();

    loop {
        // Notify connecting.
        let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
            SlackConnectionStatus::Reconnecting,
        ));

        // Get a fresh WebSocket URL.
        let ws_url = match get_ws_url(&client, &auth.app_token).await {
            Ok(url) => url,
            Err(e) => {
                error!("Failed to get Socket Mode URL: {}", e);
                let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
                    SlackConnectionStatus::AuthError(e.to_string()),
                ));
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        debug!("Connecting to Socket Mode: {}", ws_url);

        let ws_result = tokio_tungstenite::connect_async(&ws_url).await;
        let ws_stream = match ws_result {
            Ok((stream, _)) => stream,
            Err(e) => {
                error!("WebSocket connection failed: {}", e);
                let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
                    SlackConnectionStatus::Disconnected,
                ));
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        info!("Socket Mode connected");
        let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
            SlackConnectionStatus::Connected,
        ));

        let (mut ws_sink, mut ws_source) = ws_stream.split();

        // Process messages until disconnected.
        let disconnect_reason =
            process_socket_messages(&mut ws_sink, &mut ws_source, &event_tx, &our_user_id).await;

        warn!("Socket Mode disconnected: {}", disconnect_reason);
        let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
            SlackConnectionStatus::Disconnected,
        ));

        // Brief delay before reconnecting.
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Process incoming WebSocket messages from Socket Mode.
/// Returns a string describing why the connection ended.
async fn process_socket_messages(
    ws_sink: &mut SplitSink<WsStream, tokio_tungstenite::tungstenite::Message>,
    ws_source: &mut SplitStream<WsStream>,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
    our_user_id: &str,
) -> String {
    use tokio_tungstenite::tungstenite::Message;

    while let Some(msg_result) = ws_source.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => return format!("WebSocket error: {}", e),
        };

        match msg {
            Message::Text(text) => {
                if let Err(e) = handle_socket_text(&text, ws_sink, event_tx, our_user_id).await {
                    warn!("Error handling Socket Mode message: {}", e);
                }
            }
            Message::Ping(data) => {
                if let Err(e) = ws_sink.send(Message::Pong(data)).await {
                    return format!("Failed to send pong: {}", e);
                }
            }
            Message::Close(_) => {
                return "Server closed connection".to_string();
            }
            _ => {}
        }
    }

    "Stream ended".to_string()
}

/// Handle a single Socket Mode text message (JSON envelope).
async fn handle_socket_text(
    text: &str,
    ws_sink: &mut SplitSink<WsStream, tokio_tungstenite::tungstenite::Message>,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
    our_user_id: &str,
) -> Result<()> {
    use tokio_tungstenite::tungstenite::Message;

    let envelope: serde_json::Value =
        serde_json::from_str(text).context("Invalid JSON from Socket Mode")?;

    let envelope_id = envelope
        .get("envelope_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let msg_type = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Always acknowledge the envelope to prevent retries.
    if !envelope_id.is_empty() {
        let ack = serde_json::json!({ "envelope_id": envelope_id });
        ws_sink
            .send(Message::Text(ack.to_string().into()))
            .await
            .context("Failed to send ack")?;
    }

    match msg_type {
        "events_api" => {
            if let Some(payload) = envelope.get("payload") {
                debug!("Socket Mode events_api payload: {}", payload);
                handle_events_api_payload(payload, event_tx, our_user_id)?;
            }
        }
        "interactive" => {
            if let Some(payload) = envelope.get("payload") {
                debug!("Socket Mode interactive payload: {}", payload);
                handle_interactive_payload(payload, event_tx)?;
            }
        }
        "disconnect" => {
            info!("Received disconnect from Slack (normal rotation)");
        }
        "hello" => {
            info!("Socket Mode hello received — connection ready");
        }
        "slash_commands" => {
            if let Some(payload) = envelope.get("payload") {
                debug!("Socket Mode slash_commands payload: {}", payload);
                handle_slash_command_payload(payload, event_tx)?;
            }
        }
        other => {
            debug!("Unhandled Socket Mode envelope type: {}", other);
        }
    }

    Ok(())
}

/// Parse an Events API payload and emit the appropriate SlackBackgroundEvent.
fn handle_events_api_payload(
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
            &text[..text.len().min(50)]
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
fn handle_interactive_payload(
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
fn handle_slash_command_payload(
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
