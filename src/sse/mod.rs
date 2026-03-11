mod handler;
mod pollers;

pub use pollers::{spawn_provider_fetcher, spawn_session_poller};

use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::app::{BackgroundEvent, SessionInfo};

#[derive(Debug, Deserialize)]
pub(super) struct SseEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionCreatedProps {
    pub info: SessionInfo,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionDeletedProps {
    #[serde(rename = "sessionID")]
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionStatusProps {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub status: SessionStatusInfo,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionStatusInfo {
    #[serde(rename = "type")]
    pub status_type: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct MessageUpdatedProps {
    pub info: MessageInfo,
}

#[derive(Debug, Deserialize)]
pub(super) struct MessageInfo {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default, rename = "modelID")]
    #[allow(dead_code)]
    pub model_id: String,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub tokens: TokenInfo,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct TokenInfo {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub reasoning: u64,
    #[serde(default)]
    pub cache: CacheTokenInfo,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct CacheTokenInfo {
    #[serde(default)]
    pub read: u64,
    #[serde(default)]
    pub write: u64,
}

/// Spawn a background task that connects to the SSE /event endpoint for a project.
pub fn spawn_sse_listener(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    tokio::spawn(async move {
        connect_sse(tx, project_idx, project_dir).await;
    });
}

/// Connect to the SSE /event endpoint for a project and forward session events
/// as BackgroundEvents. Reconnects automatically on failure.
async fn connect_sse(
    bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let base_url = crate::app::base_url();
    loop {
        debug!(project_idx, base_url, "SSE connecting");
        match run_sse_stream(&bg_tx, project_idx, base_url, &project_dir).await {
            Ok(()) => {
                debug!(project_idx, "SSE stream ended cleanly");
            }
            Err(e) => {
                warn!(project_idx, error = %e, "SSE stream error, reconnecting in 3s");
            }
        }
        // Reconnect after delay
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn run_sse_stream(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    base_url: &str,
    project_dir: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/event", base_url))
        .header("Accept", "text/event-stream")
        .header("x-opencode-directory", project_dir)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("SSE endpoint returned status {}", response.status());
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // Process complete SSE messages (separated by double newline)
        while let Some(boundary) = buffer.find("\n\n") {
            let message = buffer[..boundary].to_string();
            buffer = buffer[boundary + 2..].to_string();

            if let Some(data) = extract_sse_data(&message) {
                if let Err(e) = handler::handle_sse_data(bg_tx, project_idx, &data) {
                    debug!(project_idx, error = %e, "Failed to parse SSE event");
                }
            }
        }
    }

    Ok(())
}

fn extract_sse_data(message: &str) -> Option<String> {
    let mut data_parts = Vec::new();
    for line in message.lines() {
        if let Some(stripped) = line.strip_prefix("data:") {
            data_parts.push(stripped.trim().to_string());
        }
    }
    if data_parts.is_empty() {
        None
    } else {
        Some(data_parts.join("\n"))
    }
}
