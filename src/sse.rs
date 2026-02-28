use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::app::{BackgroundEvent, SessionInfo};

#[derive(Debug, Deserialize)]
struct SseEvent {
    #[serde(rename = "type")]
    event_type: String,
    properties: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct SessionCreatedProps {
    info: SessionInfo,
}

#[derive(Debug, Deserialize)]
struct SessionDeletedProps {
    #[serde(rename = "sessionID")]
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct SessionStatusProps {
    #[serde(rename = "sessionID")]
    session_id: String,
    status: SessionStatusInfo,
}

#[derive(Debug, Deserialize)]
struct SessionStatusInfo {
    #[serde(rename = "type")]
    status_type: String,
}

#[derive(Debug, Deserialize)]
struct MessageUpdatedProps {
    info: MessageInfo,
}

#[derive(Debug, Deserialize)]
struct MessageInfo {
    #[serde(rename = "sessionID")]
    session_id: String,
    #[serde(default, rename = "modelID")]
    #[allow(dead_code)]
    model_id: String,
    #[serde(default)]
    cost: f64,
    #[serde(default)]
    tokens: TokenInfo,
}

#[derive(Debug, Default, Deserialize)]
struct TokenInfo {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    reasoning: u64,
    #[serde(default)]
    cache: CacheTokenInfo,
}

#[derive(Debug, Default, Deserialize)]
struct CacheTokenInfo {
    #[serde(default)]
    read: u64,
    #[serde(default)]
    write: u64,
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
                if let Err(e) = handle_sse_data(bg_tx, project_idx, &data) {
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

/// Spawn a background poller that fetches sessions via the REST API every 3s and
/// detects active sessions by comparing `time.updated` changes between polls.
pub fn spawn_session_poller(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    tokio::spawn(async move {
        let mut last_updated: HashMap<String, u64> = HashMap::new();
        let mut known_active: HashSet<String> = HashSet::new();
        let mut unchanged_count: HashMap<String, u32> = HashMap::new();
        const IDLE_THRESHOLD: u32 = 5; // 5 polls × 3s = 15s without change → idle

        let client = crate::api::ApiClient::new();
        let base_url = crate::app::base_url().to_string();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let sessions = match client.fetch_sessions(&base_url, &project_dir).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            for session in &sessions {
                let prev = last_updated.get(&session.id).copied().unwrap_or(0);
                let now = session.time.updated;

                if prev > 0 && now > prev {
                    unchanged_count.insert(session.id.clone(), 0);
                    if !known_active.contains(&session.id) {
                        known_active.insert(session.id.clone());
                        let _ = tx.send(BackgroundEvent::SseSessionBusy {
                            session_id: session.id.clone(),
                        });
                    }
                } else if known_active.contains(&session.id) {
                    let count = unchanged_count.entry(session.id.clone()).or_insert(0);
                    *count += 1;
                    if *count >= IDLE_THRESHOLD {
                        known_active.remove(&session.id);
                        unchanged_count.remove(&session.id);
                        let _ = tx.send(BackgroundEvent::SseSessionIdle {
                            project_idx,
                            session_id: session.id.clone(),
                        });
                    }
                }

                last_updated.insert(session.id.clone(), now);
            }
        }
    });
}

/// Fetch provider model limits once at startup for a project.
/// Sends ModelLimitsFetched with the max context window found across all models.
pub fn spawn_provider_fetcher(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    tokio::spawn(async move {
        let base_url = crate::app::base_url();
        let client = reqwest::Client::new();

        // Retry a few times in case the server isn't ready yet
        for attempt in 0..5 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }

            let resp = client
                .get(format!("{}/provider", base_url))
                .header("x-opencode-directory", &project_dir)
                .send()
                .await;

            let body: serde_json::Value = match resp {
                Ok(r) if r.status().is_success() => match r.json().await {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                _ => continue,
            };

            // Find the largest context window across all providers/models
            let mut max_context: u64 = 0;
            if let Some(providers) = body.as_array() {
                for provider in providers {
                    if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                        for (_model_id, model) in models {
                            if let Some(ctx) = model
                                .get("limit")
                                .and_then(|l| l.get("context"))
                                .and_then(|c| c.as_u64())
                            {
                                if ctx > max_context {
                                    max_context = ctx;
                                }
                            }
                        }
                    }
                }
            }

            if max_context > 0 {
                let _ = tx.send(BackgroundEvent::ModelLimitsFetched {
                    project_idx,
                    context_window: max_context,
                });
                debug!(project_idx, max_context, "Provider model limits fetched");
                return;
            }
        }

        // Fallback: use 200k as default
        let _ = tx.send(BackgroundEvent::ModelLimitsFetched {
            project_idx,
            context_window: 200_000,
        });
        debug!(project_idx, "Using default context window (200k)");
    });
}

fn handle_sse_data(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    data: &str,
) -> Result<()> {
    let event: SseEvent = serde_json::from_str(data)?;

    match event.event_type.as_str() {
        "session.created" => {
            let props: SessionCreatedProps = serde_json::from_value(event.properties)?;
            debug!(project_idx, session_id = %props.info.id, "SSE: session.created");
            let _ = bg_tx.send(BackgroundEvent::SseSessionCreated {
                project_idx,
                session: props.info,
            });
        }
        "session.updated" => {
            let props: SessionCreatedProps = serde_json::from_value(event.properties)?;
            debug!(project_idx, session_id = %props.info.id, "SSE: session.updated");
            let _ = bg_tx.send(BackgroundEvent::SseSessionUpdated {
                project_idx,
                session: props.info,
            });
        }
        "session.deleted" => {
            let props: SessionDeletedProps = serde_json::from_value(event.properties)?;
            debug!(project_idx, session_id = %props.session_id, "SSE: session.deleted");
            let _ = bg_tx.send(BackgroundEvent::SseSessionDeleted {
                project_idx,
                session_id: props.session_id,
            });
        }
        "session.idle" => {
            let props: SessionDeletedProps = serde_json::from_value(event.properties)?;
            debug!(project_idx, session_id = %props.session_id, "SSE: session.idle");
            let _ = bg_tx.send(BackgroundEvent::SseSessionIdle {
                project_idx,
                session_id: props.session_id,
            });
        }
        "session.status" => {
            let props: SessionStatusProps = serde_json::from_value(event.properties)?;
            debug!(project_idx, session_id = %props.session_id, status = %props.status.status_type, "SSE: session.status");
            match props.status.status_type.as_str() {
                "busy" => {
                    let _ = bg_tx.send(BackgroundEvent::SseSessionBusy {
                        session_id: props.session_id,
                    });
                }
                "idle" => {
                    let _ = bg_tx.send(BackgroundEvent::SseSessionIdle {
                        project_idx,
                        session_id: props.session_id,
                    });
                }
                _ => {}
            }
        }
        "server.connected" => {
            debug!(project_idx, "SSE: server connected");
        }
        "file.edited" => {
            info!(project_idx, raw_props = %event.properties, "SSE: file.edited raw event");
            if let Some(file) = event.properties.get("file").and_then(|v| v.as_str()) {
                debug!(project_idx, file, "SSE: file.edited - sending to handler");
                let _ = bg_tx.send(BackgroundEvent::SseFileEdited {
                    project_idx,
                    file_path: file.to_string(),
                });
            } else {
                warn!(project_idx, raw_props = %event.properties, "SSE: file.edited - no 'file' property found");
            }
        }
        "todo.updated" => {
            if let Some(session_id) = event.properties.get("sessionID").and_then(|v| v.as_str()) {
                if let Ok(todos) = serde_json::from_value::<Vec<crate::app::TodoItem>>(
                    event.properties.get("todos").cloned().unwrap_or_default(),
                ) {
                    debug!(
                        project_idx,
                        session_id,
                        count = todos.len(),
                        "SSE: todo.updated"
                    );
                    let _ = bg_tx.send(BackgroundEvent::SseTodoUpdated {
                        session_id: session_id.to_string(),
                        todos,
                    });
                }
            }
        }
        _ => {
            // Try to handle message.updated for token/cost tracking
            if event.event_type == "message.updated" {
                if let Ok(props) = serde_json::from_value::<MessageUpdatedProps>(event.properties) {
                    let info = &props.info;
                    debug!(
                        session_id = %info.session_id,
                        cost = info.cost,
                        input_tokens = info.tokens.input,
                        output_tokens = info.tokens.output,
                        "SSE: message.updated with cost/token data"
                    );
                    let _ = bg_tx.send(BackgroundEvent::SseMessageUpdated {
                        session_id: info.session_id.clone(),
                        cost: info.cost,
                        input_tokens: info.tokens.input,
                        output_tokens: info.tokens.output,
                        reasoning_tokens: info.tokens.reasoning,
                        cache_read: info.tokens.cache.read,
                        cache_write: info.tokens.cache.write,
                    });
                }
            }
            // Ignore other events (permission.*, etc.)
        }
    }

    Ok(())
}
