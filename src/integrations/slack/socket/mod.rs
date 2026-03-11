//! Slack Socket Mode WebSocket connection and event handling.

use std::time::Duration;

use anyhow::{Context, Result};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::integrations::slack::auth::SlackAuth;
use crate::integrations::slack::types::{SlackBackgroundEvent, SlackConnectionStatus};

mod events;
mod interactive;

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
                events::handle_events_api_payload(payload, event_tx, our_user_id)?;
            }
        }
        "interactive" => {
            if let Some(payload) = envelope.get("payload") {
                debug!("Socket Mode interactive payload: {}", payload);
                interactive::handle_interactive_payload(payload, event_tx)?;
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
                interactive::handle_slash_command_payload(payload, event_tx)?;
            }
        }
        other => {
            debug!("Unhandled Socket Mode envelope type: {}", other);
        }
    }

    Ok(())
}
