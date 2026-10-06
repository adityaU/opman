//! `GET /api/stream/ws` — every SSE stream of one frontend instance over one WebSocket.
//!
//! On a plain-HTTP origin a browser allows six HTTP/1.1 connections per origin, shared
//! by the page and every same-origin iframe. Each app instance keeps several SSE streams
//! open (app events, session events, one per terminal, one per browser pane, system
//! stats), so a workspace with cross-server panes (each an iframe running its own
//! instance) fills the pool and every further request hangs. WebSockets are not counted
//! against that pool, so the frontend multiplexes all its streams over this one socket.
//!
//! Protocol, JSON text frames:
//! - client → server `{"op":"sub","id":<u32>,"path":"/api/events?x=y"}` and
//!   `{"op":"unsub","id":<u32>}`.
//! - server → client `{"id":..,"op":"open"}` once the stream answered,
//!   `{"id":..,"event":<name or "message">,"data":<string>,"lastId"?:<string>}` per event,
//!   `{"id":..,"op":"end","status"?:<http status>}` when it ends (no status = the stream
//!   finished; a status = it was refused, like a non-200 or non-SSE answer), and
//!   `{"op":"ping"}` every few seconds so the client can tell a dead socket from a quiet one.
//!
//! A subscription is served by dispatching an internal `GET` into the app's own router
//! carrying the upgrade request's credentials, so it is authorized exactly like the SSE
//! request the browser would have made. Unsubscribing (or closing the socket) drops the
//! internal response body, which releases whatever the endpoint holds for a viewer.

use std::sync::{Arc, OnceLock};

use axum::extract::ws::WebSocketUpgrade;
use axum::http::{HeaderMap, Uri};
use axum::response::Response;
use axum::{Extension, Router};

use super::auth::AuthUser;

mod protocol;
mod session;
mod sse_parse;
mod subscription;

/// The app's own router, filled in right after it is built so the handler can dispatch
/// subscriptions into it.
///
/// The router carries this slot as an extension, so the two reference each other; the
/// router lives as long as the server, so the cycle costs nothing in production.
#[derive(Clone, Default)]
pub(crate) struct RouterSlot(Arc<OnceLock<Router>>);

impl RouterSlot {
    /// Publish the finished router. Only the first call has an effect.
    pub(crate) fn fill(&self, router: Router) {
        let _ = self.0.set(router);
    }

    fn get(&self) -> Option<Router> {
        self.0.get().cloned()
    }
}

pub(crate) async fn stream_ws(
    _auth: AuthUser,
    Extension(slot): Extension<RouterSlot>,
    headers: HeaderMap,
    uri: Uri,
    upgrade: WebSocketUpgrade,
) -> Response {
    let credentials = Arc::new(subscription::credentials(&headers, &uri));
    upgrade.on_upgrade(move |socket| session::run(socket, slot, credentials))
}

#[cfg(test)]
#[path = "sse_parse_tests.rs"]
mod sse_parse_tests;

#[cfg(test)]
#[path = "stream_mux_tests.rs"]
mod stream_mux_tests;

#[cfg(test)]
#[path = "stream_mux_remote_tests.rs"]
mod stream_mux_remote_tests;
