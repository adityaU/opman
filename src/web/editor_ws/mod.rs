//! The editor's binary channel.
//!
//! One authenticated WebSocket per editor pane carrying MessagePack frames, in
//! place of a POST per query. Two things that HTTP could not give the editor:
//! every request shares one socket rather than competing for the browser's
//! per-origin connections, and a request can be withdrawn once its answer stops
//! mattering — which is most of them, when the pointer is moving.

mod dispatch;
mod protocol;
mod session;

use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;

use super::auth::check_auth_manual;
use super::error::WebError;
use super::types::{ServerState, SseTokenQuery};

pub async fn websocket_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }
    Ok(ws.on_upgrade(move |socket| session::run(socket, state)))
}
