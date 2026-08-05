//! HTTP access logging for the web server.
//!
//! Emits one line when a request arrives (`--> GET /api/state`) and one when it
//! finishes (`<-- 200 GET /api/state 3ms`), correlated by a per-request `rid`.
//! Bodies are never read or logged — only method, path, status, elapsed time
//! and the byte counts already present in `content-length` headers. Query
//! strings are omitted too, since some carry user text (search queries).
//!
//! The layer also catches requests that never produce a response: axum drops
//! the handler future when the client disconnects mid-request, which is what
//! cloudflared surfaces as `Incoming request ended abruptly: context canceled`.
//! A drop guard turns that into an explicit `client canceled` line, so a
//! dropped POST is visible on our side of the tunnel instead of only in the
//! tunnel's own log.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::extract::Request;
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;

/// Tracing target for every line this module emits, so access logging can be
/// tuned independently: `RUST_LOG=opman::http=warn` quiets the normal traffic
/// while still surfacing cancellations and 5xx.
const TARGET: &str = "opman::http";

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

/// How chatty a given path should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Traffic {
    /// Regular REST call — logged at info.
    Api,
    /// Long-lived SSE stream or websocket upgrade. Logged at debug: these are
    /// opened constantly by the UI and their disconnects are routine.
    Stream,
    /// Static frontend asset served by the fallback. Logged at debug.
    Asset,
}

/// Classify a request path for log verbosity.
///
/// Streams are matched by suffix because they live under several prefixes
/// (`/api/events`, `/api/session/events`, `/api/pty/stream`, `/api/mcp/ws`).
pub(crate) fn classify(path: &str) -> Traffic {
    let api = path.starts_with("/api/") || path.starts_with("/internal/") || path == "/health";
    if !api {
        return Traffic::Asset;
    }
    if path.ends_with("/events") || path.ends_with("/stream") || path.ends_with("/ws") {
        return Traffic::Stream;
    }
    Traffic::Api
}

/// Read `content-length` as a number, or 0 when absent (chunked / no body).
pub(crate) fn body_bytes(headers: &HeaderMap) -> u64 {
    headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Logs a cancellation if the request future is dropped before completing.
///
/// `done` is set once a response exists; the `Drop` impl only fires for the
/// abandoned case.
struct Pending {
    rid: u64,
    method: Method,
    path: String,
    start: Instant,
    traffic: Traffic,
    done: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let ms = self.start.elapsed().as_millis();
        // A stream ending this way is just the client going away; a REST call
        // ending this way means we may have done the work and thrown the ack
        // away, which is worth a warning.
        if self.traffic == Traffic::Api {
            tracing::warn!(
                target: TARGET,
                rid = self.rid,
                method = %self.method,
                path = %self.path,
                ms,
                "<x- client canceled before response"
            );
        } else {
            tracing::debug!(
                target: TARGET,
                rid = self.rid,
                method = %self.method,
                path = %self.path,
                ms,
                "<x- client canceled before response"
            );
        }
    }
}

/// Access-log middleware. Add as the outermost layer so it also sees responses
/// produced by rejections (body-limit, auth) and the static-file fallback.
pub(super) async fn log_requests(req: Request, next: Next) -> Response {
    let rid = REQUEST_SEQ.fetch_add(1, Ordering::Relaxed);
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let traffic = classify(&path);
    let req_bytes = body_bytes(req.headers());

    if traffic == Traffic::Api {
        tracing::info!(target: TARGET, rid, method = %method, path = %path, req_bytes, "--> request");
    } else {
        tracing::debug!(target: TARGET, rid, method = %method, path = %path, req_bytes, "--> request");
    }

    let mut pending = Pending {
        rid,
        method: method.clone(),
        path: path.clone(),
        start: Instant::now(),
        traffic,
        done: false,
    };

    let response = next.run(req).await;
    pending.done = true;

    let status = response.status();
    let ms = pending.start.elapsed().as_millis();
    let resp_bytes = body_bytes(response.headers());

    if status.is_server_error() {
        tracing::warn!(target: TARGET, rid, method = %method, path = %path, status = status.as_u16(), ms, resp_bytes, "<-- response");
    } else if status.is_client_error() || traffic == Traffic::Api {
        tracing::info!(target: TARGET, rid, method = %method, path = %path, status = status.as_u16(), ms, resp_bytes, "<-- response");
    } else {
        tracing::debug!(target: TARGET, rid, method = %method, path = %path, status = status.as_u16(), ms, resp_bytes, "<-- response");
    }

    response
}

#[cfg(test)]
#[path = "request_log_tests.rs"]
mod request_log_tests;
