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
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;

use super::auth::check_auth_manual;
use super::error::WebError;
use super::types::*;

#[path = "sse_render.rs"]
mod render;
use render::{render_editor_event, render_web_event};

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
