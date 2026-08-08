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

/// Map a broadcast [`WebEvent`] to the SSE event that should be sent to the
/// client, or `None` when the event produces no output (`Noop`, or a payload
/// that fails to serialize). Pure helper extracted from `events_stream` so the
/// per-event mapping is unit-testable.
fn render_web_event(event: &WebEvent) -> Option<SseEvent> {
    match event {
        WebEvent::Noop => None,
        WebEvent::StateChanged => Some(SseEvent::default().event("state_changed").data("")),
        WebEvent::McpServersChanged => {
            Some(SseEvent::default().event("mcp_servers_changed").data(""))
        }
        WebEvent::AcpAgentsChanged => {
            Some(SseEvent::default().event("acp_agents_changed").data(""))
        }
        WebEvent::SessionBusy { session_id } => Some(
            SseEvent::default()
                .event("session_busy")
                .data(session_id.clone()),
        ),
        WebEvent::SessionIdle { session_id } => Some(
            SseEvent::default()
                .event("session_idle")
                .data(session_id.clone()),
        ),
        WebEvent::StatsUpdated(stats) => serde_json::to_string(stats)
            .ok()
            .map(|json| SseEvent::default().event("stats_updated").data(json)),
        WebEvent::ThemeChanged(colors) => serde_json::to_string(colors)
            .ok()
            .map(|json| SseEvent::default().event("theme_changed").data(json)),
        WebEvent::WatcherStatusChanged(watcher_event) => serde_json::to_string(watcher_event)
            .ok()
            .map(|json| SseEvent::default().event("watcher_status").data(json)),
        WebEvent::McpEditorOpen { path, line } => {
            let payload = serde_json::json!({ "path": path, "line": line });
            Some(
                SseEvent::default()
                    .event("mcp_editor_open")
                    .data(payload.to_string()),
            )
        }
        WebEvent::McpEditorNavigate { line } => {
            let payload = serde_json::json!({ "line": line });
            Some(
                SseEvent::default()
                    .event("mcp_editor_navigate")
                    .data(payload.to_string()),
            )
        }
        WebEvent::McpTerminalFocus { id } => Some(
            SseEvent::default()
                .event("mcp_terminal_focus")
                .data(id.clone()),
        ),
        WebEvent::McpAgentActivity { tool, active } => {
            let payload = serde_json::json!({ "tool": tool, "active": active });
            Some(
                SseEvent::default()
                    .event("mcp_agent_activity")
                    .data(payload.to_string()),
            )
        }
        WebEvent::PresenceChanged(snapshot) => serde_json::to_string(snapshot)
            .ok()
            .map(|json| SseEvent::default().event("presence_changed").data(json)),
        WebEvent::RoutineUpdated => Some(SseEvent::default().event("routine_updated").data("")),
        WebEvent::KanbanTaskUpdated {
            project_path,
            task_id,
        } => {
            let payload = serde_json::json!({ "project_path": project_path, "task_id": task_id });
            Some(
                SseEvent::default()
                    .event("kanban_task")
                    .data(payload.to_string()),
            )
        }
        WebEvent::KanbanBoardUpdated { project_path } => {
            let payload = serde_json::json!({ "project_path": project_path });
            Some(
                SseEvent::default()
                    .event("kanban_board")
                    .data(payload.to_string()),
            )
        }
        WebEvent::Toast { message, level } => {
            let payload = serde_json::json!({ "message": message, "level": level });
            Some(SseEvent::default().event("toast").data(payload.to_string()))
        }
        WebEvent::SessionError { session_id, .. } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_error")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionInputNeeded { session_id } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_input_needed")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionInputCleared { session_id } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_input_cleared")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionUnseen { session_id, count } => {
            let payload = serde_json::json!({ "session_id": session_id, "count": count });
            Some(
                SseEvent::default()
                    .event("session_unseen")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionSeen { session_id } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_seen")
                    .data(payload.to_string()),
            )
        }
    }
}

/// Map an [`EditorEvent`] to the `file_changed` SSE event, or `None` if it
/// fails to serialize. Pure helper extracted from `editor_events_stream`.
fn render_editor_event(event: &EditorEvent) -> Option<SseEvent> {
    serde_json::to_string(event)
        .ok()
        .map(|json| SseEvent::default().event("file_changed").data(json))
}

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

    let replay = params.replay;

    // Stream that polls the raw output buffer at ~20fps.
    // The frontend coalesces output via requestAnimationFrame, so even
    // if multiple SSE events arrive within a single frame they are
    // processed as a single batch. This interval provides a good
    // balance between latency and CPU usage.
    let stream = async_stream::stream! {
        // A re-attaching tab leads with the retained scrollback so the PTY it
        // rejoined repaints instead of coming back blank. `snapshot` also seeks
        // the reader to the tip, so the poll loop below never repeats it.
        if replay == Replay::Yes {
            let history = output.snapshot();
            if !history.is_empty() {
                let encoded = BASE64.encode(&history);
                yield Ok::<_, Infallible>(SseEvent::default().event("output").data(encoded));
            }
        }

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

#[cfg(test)]
#[path = "sse_tests.rs"]
mod sse_tests;

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
                            if let Some(sse) = render_web_event(&event) {
                                yield Ok::<_, Infallible>(sse);
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

// ── Editor events stream ────────────────────────────────────────────

/// GET /api/editor/events — SSE stream of file-change notifications.
///
/// Separate from `/api/events` and `/api/session/events` so the editor can
/// react to file modifications without processing unrelated traffic.
pub async fn editor_events_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    let mut editor_rx = state.editor_tx.subscribe();

    let stream = async_stream::stream! {
        yield Ok::<_, Infallible>(
            SseEvent::default().event("heartbeat").data(""),
        );

        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        heartbeat_interval.tick().await;

        loop {
            tokio::select! {
                result = editor_rx.recv() => {
                    match result {
                        Ok(event) => {
                            if let Some(sse) = render_editor_event(&event) {
                                yield Ok::<_, Infallible>(sse);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Client fell behind — send a generic refresh hint.
                            yield Ok::<_, Infallible>(
                                SseEvent::default().event("refresh").data(""),
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

// ── System stats stream ─────────────────────────────────────────────

/// GET /api/system/stats/stream — SSE stream of system metrics at ~2s intervals.
///
/// Spawns a dedicated blocking thread that owns a persistent `sysinfo::System`
/// instance so CPU usage deltas are computed correctly across ticks.
pub async fn system_stats_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    // Channel to bridge blocking thread → async SSE stream.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(4);

    // Persistent blocking thread that keeps the System alive for accurate CPU deltas.
    tokio::task::spawn_blocking(move || {
        use super::handlers::system_handlers::collect_system_stats_reuse;
        use sysinfo::{Disks, Networks, System};

        let mut sys = System::new_all();
        // First refresh to seed CPU baseline — usage will be 0 here.
        sys.refresh_all();
        // Sleep briefly so the second sample produces real CPU numbers.
        std::thread::sleep(std::time::Duration::from_millis(500));

        loop {
            sys.refresh_all();
            let disks = Disks::new_with_refreshed_list();
            let networks = Networks::new_with_refreshed_list();
            let stats = collect_system_stats_reuse(&sys, &disks, &networks);
            if let Ok(json) = serde_json::to_string(&stats) {
                if tx.blocking_send(json).is_err() {
                    // Receiver dropped (client disconnected) — exit thread.
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });

    let stream = async_stream::stream! {
        while let Some(json) = rx.recv().await {
            yield Ok::<_, Infallible>(
                SseEvent::default().event("system_stats").data(json),
            );
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[cfg(test)]
#[path = "sse_stream_tests.rs"]
mod sse_stream_tests;
