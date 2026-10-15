//! A web-owned PTY's live connection to the browser.
//!
//! Two transports, one rule: output goes out the moment the reader thread pushes it.
//! The stream used to look for new bytes every 50 ms, which put up to 50 ms on every
//! echoed keystroke and capped full-screen programs at 20 frames a second.
//!
//! - `/pty/ws` — the terminal's own WebSocket. Raw bytes both ways: keystrokes in as
//!   binary frames, output out as binary frames. No request per key, no base64, no JSON,
//!   and ordering comes from the socket itself. What the terminal uses.
//! - `/pty/stream` — the SSE stream, kept for clients that cannot open the socket. Its
//!   input still goes through `POST /pty/write`.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use futures::{SinkExt, StreamExt};

use super::auth::check_auth_manual;
use super::error::WebError;
use super::pty_manager::{RawOutputBuffer, WebPtyHandle};
use super::types::{Replay, ServerState, SseTokenQuery};

/// The PTY a request names, once the caller is known to be allowed it.
async fn open(
    state: &ServerState,
    headers: &HeaderMap,
    params: &SseTokenQuery,
) -> Result<(String, RawOutputBuffer), WebError> {
    // Manual auth: neither EventSource nor WebSocket can set headers.
    if !check_auth_manual(state, headers, &params.token) {
        return Err(WebError::Unauthorized);
    }
    let id = params
        .id
        .clone()
        .ok_or(WebError::BadRequest("Missing 'id' parameter".into()))?;
    let output = state
        .pty_mgr
        .get_output(&id)
        .await
        .ok_or(WebError::NotFound("PTY not found or not spawned yet"))?;
    Ok((id, output))
}

/// A re-attaching view leads with the retained scrollback, so a PTY it rejoins repaints
/// instead of coming back blank. `snapshot` also seeks the reader to the tip.
fn replay(output: &RawOutputBuffer, replay: Replay) -> Option<Vec<u8>> {
    if replay != Replay::Yes {
        return None;
    }
    let history = output.snapshot();
    (!history.is_empty()).then_some(history)
}

pub async fn terminal_socket(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, WebError> {
    let (id, output) = open(&state, &headers, &params).await?;
    let pty = state.pty_mgr.clone();
    let history = replay(&output, params.replay);
    Ok(ws.on_upgrade(move |socket| run_socket(socket, pty, id, output, history)))
}

async fn run_socket(
    socket: WebSocket,
    pty: WebPtyHandle,
    id: String,
    output: RawOutputBuffer,
    history: Option<Vec<u8>>,
) {
    let (mut sender, mut receiver) = socket.split();
    if let Some(history) = history {
        if sender.send(Message::Binary(history.into())).await.is_err() {
            return;
        }
    }
    loop {
        tokio::select! {
            () = output.arrived() => {
                let bytes = output.drain_new();
                if !bytes.is_empty() && sender.send(Message::Binary(bytes.into())).await.is_err() {
                    break;
                }
            }
            frame = receiver.next() => {
                let input = match frame {
                    Some(Ok(Message::Binary(bytes))) => bytes.to_vec(),
                    Some(Ok(Message::Text(text))) => text.as_bytes().to_vec(),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    // Closed, errored, or the PTY is gone: the view detaches, the
                    // program keeps running.
                    _ => break,
                };
                if !pty.write(&id, input).await {
                    break;
                }
            }
        }
    }
}

pub async fn terminal_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
) -> Result<impl IntoResponse, WebError> {
    let (_, output) = open(&state, &headers, &params).await?;
    let history = replay(&output, params.replay);
    let stream = async_stream::stream! {
        if let Some(history) = history {
            yield Ok::<_, Infallible>(SseEvent::default().event("output").data(BASE64.encode(&history)));
        }
        loop {
            output.arrived().await;
            let bytes = output.drain_new();
            if !bytes.is_empty() {
                yield Ok(SseEvent::default().event("output").data(BASE64.encode(&bytes)));
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[cfg(test)]
#[path = "pty_stream_tests.rs"]
mod pty_stream_tests;
