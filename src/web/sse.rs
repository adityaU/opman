//! Server-Sent Events (SSE) streaming endpoints.
//!
//! Two SSE streams are provided:
//!
//! - **Terminal stream** (`/api/pty/stream?id=<pty_id>`)
//!   Polls a web-owned PTY's raw output buffer at 20fps and sends
//!   base64-encoded raw VT100 bytes. xterm.js handles rendering natively.
//!
//! - **App events** (`/api/events`)
//!   Broadcasts state changes, session busy/idle transitions, and stats updates
//!   using a `watch` channel from the independent `WebStateHandle`.
//!
//! Both endpoints accept auth via `Authorization: Bearer <token>` header or
//! `?token=<jwt>` query parameter (since `EventSource` doesn't support headers).

use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use super::auth::check_auth_manual;
use super::error::WebError;
use super::types::*;

// ── Terminal output stream (raw bytes from web-owned PTY) ───────────

pub async fn terminal_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
) -> Result<impl IntoResponse, WebError> {
    // SSE endpoints use manual auth check (EventSource can't set headers)
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    let pty_id = params
        .id
        .ok_or(WebError::BadRequest("Missing 'id' parameter".into()))?;

    // Get the raw output buffer from the web PTY manager
    let output = state
        .pty_mgr
        .get_output(&pty_id)
        .await
        .ok_or(WebError::NotFound("PTY not found or not spawned yet"))?;

    // Stream that polls the raw output buffer at 20fps
    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_millis(50));

        loop {
            interval.tick().await;

            // Only check when dirty flag is set (new output arrived)
            if !output.dirty.load(Ordering::Acquire) {
                continue;
            }

            // Drain new bytes from the buffer
            let new_bytes = output.drain_new();
            if !new_bytes.is_empty() {
                let encoded = BASE64.encode(&new_bytes);
                yield Ok::<_, Infallible>(SseEvent::default().event("output").data(encoded));
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

// ── App events stream ───────────────────────────────────────────────

pub async fn events_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    let mut event_rx = state.event_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    match &event {
                        WebEvent::Noop => continue,
                        WebEvent::StateChanged => {
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("state_changed").data(""),
                            );
                        }
                        WebEvent::SessionBusy { session_id } => {
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("session_busy").data(session_id.clone()),
                            );
                        }
                        WebEvent::SessionIdle { session_id } => {
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("session_idle").data(session_id.clone()),
                            );
                        }
                        WebEvent::StatsUpdated(stats) => {
                            if let Ok(json) = serde_json::to_string(stats) {
                                yield Ok::<_, Infallible>(
                                    SseEvent::default().event("stats_updated").data(json),
                                );
                            }
                        }
                        WebEvent::ThemeChanged(colors) => {
                            if let Ok(json) = serde_json::to_string(colors) {
                                yield Ok::<_, Infallible>(
                                    SseEvent::default().event("theme_changed").data(json),
                                );
                            }
                        }
                        WebEvent::WatcherStatusChanged(watcher_event) => {
                            if let Ok(json) = serde_json::to_string(watcher_event) {
                                yield Ok::<_, Infallible>(
                                    SseEvent::default().event("watcher_status").data(json),
                                );
                            }
                        }
                        WebEvent::McpEditorOpen { path, line } => {
                            let payload = serde_json::json!({ "path": path, "line": line });
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("mcp_editor_open").data(payload.to_string()),
                            );
                        }
                        WebEvent::McpEditorNavigate { line } => {
                            let payload = serde_json::json!({ "line": line });
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("mcp_editor_navigate").data(payload.to_string()),
                            );
                        }
                        WebEvent::McpTerminalFocus { id } => {
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("mcp_terminal_focus").data(id.clone()),
                            );
                        }
                        WebEvent::McpAgentActivity { tool, active } => {
                            let payload = serde_json::json!({ "tool": tool, "active": active });
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("mcp_agent_activity").data(payload.to_string()),
                            );
                        }
                        WebEvent::ActivityEvent(activity) => {
                            if let Ok(json) = serde_json::to_string(activity) {
                                yield Ok::<_, Infallible>(
                                    SseEvent::default().event("activity_event").data(json),
                                );
                            }
                        }
                        WebEvent::PresenceChanged(snapshot) => {
                            if let Ok(json) = serde_json::to_string(snapshot) {
                                yield Ok::<_, Infallible>(
                                    SseEvent::default().event("presence_changed").data(json),
                                );
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // Client fell behind — some events were dropped. Send a
                    // state_changed so the client does a full refresh.
                    tracing::debug!("SSE client lagged by {} events, sending state_changed", n);
                    yield Ok::<_, Infallible>(
                        SseEvent::default().event("state_changed").data(""),
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

// ── Session event stream (proxy from opencode server) ───────────────

/// GET /api/session/events — proxy the opencode server's SSE `/event` stream
/// to the web client. Forwards all events as-is (message.updated, session.status,
/// permission.asked, question.asked, file.edited, todo.updated, etc.).
pub async fn session_events_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SessionSseQuery>,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    let project_dir = match &params.project_dir {
        Some(d) => d.clone(),
        None => state
            .web_state
            .get_working_dir()
            .await
            .map(|p| p.to_string_lossy().to_string())
            .ok_or(WebError::BadRequest("No active project".into()))?,
    };

    let base = crate::app::base_url().to_string();

    let stream = async_stream::stream! {
        use futures::StreamExt;

        let client = reqwest::Client::new();
        let resp = match client
            .get(format!("{}/event", base))
            .header("Accept", "text/event-stream")
            .header("x-opencode-directory", &project_dir)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                yield Ok::<_, Infallible>(
                    SseEvent::default()
                        .event("error")
                        .data(format!("Upstream returned {}", status)),
                );
                return;
            }
            Err(e) => {
                yield Ok::<_, Infallible>(
                    SseEvent::default()
                        .event("error")
                        .data(format!("Connection failed: {}", e)),
                );
                return;
            }
        };

        let mut byte_stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => break,
            };
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE messages (separated by double newline)
            while let Some(boundary) = buffer.find("\n\n") {
                let message: String = buffer.drain(..boundary).collect();
                buffer.drain(..2); // consume the "\n\n" separator

                // Forward the raw SSE data as an "opencode" event
                let mut data_parts = Vec::new();
                for line in message.lines() {
                    if let Some(stripped) = line.strip_prefix("data:") {
                        data_parts.push(stripped.trim().to_string());
                    }
                }
                if !data_parts.is_empty() {
                    let data = data_parts.join("\n");
                    yield Ok::<_, Infallible>(
                        SseEvent::default().event("opencode").data(data),
                    );
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
