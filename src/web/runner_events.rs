//! Event plumbing for non-default runners.
//!
//! The default runner has its own reconnecting listener in the web state. Every
//! other runner reaches the same handlers through here — over HTTP SSE for
//! out-of-process engines, or straight off a broadcast channel for ones opman
//! hosts itself. Both feeds are kept alive for the life of the process: a
//! runner that starts late, restarts, or floods is the normal case, and a feed
//! that quietly ends takes that runner's session status with it.

use tokio::sync::broadcast;

use super::WebStateHandle;

/// How long to wait before re-reading the project list and reopening streams.
const RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Follow one runner's HTTP SSE endpoint for as long as the process runs.
///
/// The project list is re-read on every cycle, so a project added after startup
/// picks up the runner's events without a restart.
pub(super) fn spawn_runner_event_forwarder(
    endpoint: String,
    runner: String,
    tx: broadcast::Sender<String>,
    web_state: WebStateHandle,
) {
    tokio::spawn(async move {
        let mut streams: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        loop {
            for stream in streams.drain(..) {
                stream.abort();
            }
            for directory in web_state.project_directories().await {
                streams.push(tokio::spawn(follow_runner_stream(
                    endpoint.clone(),
                    directory,
                    runner.clone(),
                    tx.clone(),
                    web_state.clone(),
                )));
            }
            tokio::time::sleep(RECONNECT_INTERVAL).await;
        }
    });
}

/// Read one `(endpoint, directory)` SSE stream until it ends or errors.
///
/// Failures are logged at debug: a runner being unreachable is expected between
/// its restarts, and the caller reopens the stream on the next cycle.
async fn follow_runner_stream(
    endpoint: String,
    directory: String,
    runner: String,
    tx: broadcast::Sender<String>,
    web_state: WebStateHandle,
) {
    use futures::StreamExt;

    let client = reqwest::Client::new();
    let response = match client
        .get(&endpoint)
        .header("Accept", "text/event-stream")
        .header("x-opencode-directory", &directory)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            tracing::debug!(%endpoint, status = %response.status(), "runner SSE unavailable");
            return;
        }
        Err(error) => {
            tracing::debug!(%endpoint, %error, "runner SSE connection failed");
            return;
        }
    };

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else { break };
        buffer.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(boundary) = buffer.find("\n\n") {
            let message: String = buffer.drain(..boundary).collect();
            buffer.drain(..2);
            let data = message
                .lines()
                .filter_map(|line| line.strip_prefix("data:").map(str::trim))
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() {
                continue;
            }
            let _ = tx.send(data.clone());
            super::label_created_session(&web_state, &data, &runner).await;
            web_state.handle_runner_event(&data, &directory).await;
        }
    }
}

/// Forward an in-process runner's broadcast events into the web state.
///
/// A busy turn emits faster than this loop consumes, so the channel lagging is
/// routine. Dropping events costs a few UI updates the next status sweep will
/// repair; exiting the loop would cost every future event from that runner, so
/// a lag is skipped rather than fatal.
pub(super) fn spawn_runner_event_receiver(
    mut receiver: broadcast::Receiver<String>,
    runner: String,
    tx: broadcast::Sender<String>,
    web_state: WebStateHandle,
) {
    tokio::spawn(async move {
        loop {
            let data = match receiver.recv().await {
                Ok(data) => data,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::debug!(%runner, skipped, "runner event channel lagged");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return,
            };
            let _ = tx.send(data.clone());
            super::label_created_session(&web_state, &data, &runner).await;
            let session_id = serde_json::from_str::<serde_json::Value>(&data)
                .ok()
                .and_then(|event| {
                    event
                        .pointer("/properties/sessionID")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            let directory = match session_id {
                Some(session_id) => web_state
                    .directory_for_session(&session_id)
                    .await
                    .unwrap_or_default(),
                None => String::new(),
            };
            web_state.handle_runner_event(&data, &directory).await;
        }
    });
}
