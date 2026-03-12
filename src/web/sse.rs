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
        // Send an initial heartbeat so the frontend knows the connection is live.
        yield Ok::<_, Infallible>(
            SseEvent::default().event("heartbeat").data(""),
        );

        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        // The first tick fires immediately — skip it since we already sent one above.
        heartbeat_interval.tick().await;

        loop {
            tokio::select! {
                result = event_rx.recv() => {
                    match result {
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
                                WebEvent::MissionUpdated { mission } => {
                                    yield Ok::<_, Infallible>(
                                        SseEvent::default().event("mission_updated").data(mission.to_string()),
                                    );
                                }
                                WebEvent::Toast { message, level } => {
                                    let payload = serde_json::json!({ "message": message, "level": level });
                                    yield Ok::<_, Infallible>(
                                        SseEvent::default().event("toast").data(payload.to_string()),
                                    );
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
                _ = heartbeat_interval.tick() => {
                    yield Ok::<_, Infallible>(
                        SseEvent::default().event("heartbeat").data(""),
                    );
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

// ── Session event stream (re-broadcast from internal SSE listener) ──

/// GET /api/session/events — forward opencode server events to the web client.
///
/// Instead of opening a separate upstream SSE connection (the opencode server
/// may limit concurrent SSE consumers per project), this endpoint subscribes
/// to the `raw_sse_tx` broadcast channel that is fed by the `web_state`'s
/// internal SSE listener.  Every raw event JSON string is forwarded as an
/// `"opencode"` SSE event to the browser.
pub async fn session_events_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SessionSseQuery>,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    let mut raw_rx = state.raw_sse_tx.subscribe();

    let stream = async_stream::stream! {
        tracing::info!("Session SSE: web client subscribed to raw_sse_tx broadcast");

        // Send an initial heartbeat so the frontend knows the connection is live.
        yield Ok::<_, Infallible>(
            SseEvent::default().event("heartbeat").data(""),
        );

        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        // The first tick fires immediately — skip it since we already sent one above.
        heartbeat_interval.tick().await;

        loop {
            tokio::select! {
                result = raw_rx.recv() => {
                    match result {
                        Ok(data) => {
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("opencode").data(data),
                            );
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!("Session SSE: web client lagged by {} events", n);
                            // Tell the frontend it missed events so it can do a full refresh.
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("lagged").data(n.to_string()),
                            );
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            tracing::info!("Session SSE: raw_sse_tx channel closed, ending stream");
                            break;
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok::<_, Infallible>(
                        SseEvent::default().event("heartbeat").data(""),
                    );
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
