//! One subscription: an internal `GET` into the app's router, its SSE body parsed and
//! forwarded as frames until it ends or the subscription is dropped.

use std::sync::Arc;

use axum::body::Body;
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, COOKIE};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, Uri};
use futures::StreamExt;
use tokio::sync::mpsc;
use tower::ServiceExt;

use super::sse_parse::SseParser;
use super::{protocol, RouterSlot};

/// The only prefix a subscription may reach.
const API_PREFIX: &str = "/api/";
/// This endpoint itself — subscribing to it would only recurse.
const SELF_PATH: &str = "/api/stream/ws";

/// The credentials of the upgrade request, as headers for every internal request.
///
/// A `?token=` on the upgrade URL becomes a bearer header, since the internal request's
/// own query belongs to the subscribed path.
pub(super) fn credentials(headers: &HeaderMap, uri: &Uri) -> HeaderMap {
    let mut out = HeaderMap::new();
    for name in [COOKIE, AUTHORIZATION] {
        for value in headers.get_all(&name) {
            out.append(name.clone(), value.clone());
        }
    }
    if out.contains_key(AUTHORIZATION) {
        return out;
    }
    let token = uri.query().and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .find(|(k, _)| k == "token")
            .map(|(_, v)| v.into_owned())
    });
    if let Some(value) = token.and_then(|t| HeaderValue::from_str(&format!("Bearer {t}")).ok()) {
        out.insert(AUTHORIZATION, value);
    }
    out
}

/// Serve subscription `id` until its stream ends or the task is aborted.
pub(super) async fn pump(
    id: u32,
    path: String,
    slot: RouterSlot,
    credentials: Arc<HeaderMap>,
    out: mpsc::Sender<String>,
) {
    let refused = match open(&path, &slot, &credentials).await {
        Ok(body) => {
            if out.send(protocol::open(id)).await.is_err() {
                return;
            }
            if !forward(id, body, &out).await {
                return;
            }
            None
        }
        Err(status) => Some(status),
    };
    let _ = out.send(protocol::end(id, refused)).await;
}

/// Dispatch the internal request; the body when it answered with an event stream.
async fn open(path: &str, slot: &RouterSlot, credentials: &HeaderMap) -> Result<Body, StatusCode> {
    let uri = allowed_uri(path).ok_or(StatusCode::BAD_REQUEST)?;
    let router = slot.get().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let mut request = Request::get(uri)
        .body(Body::empty())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    *request.headers_mut() = credentials.clone();
    request
        .headers_mut()
        .insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
    let Ok(response) = router.oneshot(request).await;
    if !response.status().is_success() {
        return Err(response.status());
    }
    if !is_event_stream(response.headers()) {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    Ok(response.into_body())
}

/// A same-server `/api/...` path, never this endpoint or an absolute URL.
pub(super) fn allowed_uri(path: &str) -> Option<Uri> {
    if !path.starts_with(API_PREFIX) {
        return None;
    }
    let route = path.split(['?', '#']).next().unwrap_or_default();
    if route.trim_end_matches('/') == SELF_PATH {
        return None;
    }
    let uri: Uri = path.parse().ok()?;
    if uri.scheme().is_some() || uri.authority().is_some() {
        return None;
    }
    Some(uri)
}

fn is_event_stream(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim_start().starts_with("text/event-stream"))
}

/// Forward events until the body ends. `false` when the socket went away first.
async fn forward(id: u32, body: Body, out: &mpsc::Sender<String>) -> bool {
    let mut parser = SseParser::default();
    let mut chunks = body.into_data_stream();
    while let Some(chunk) = chunks.next().await {
        let Ok(chunk) = chunk else {
            return true;
        };
        parser.push(chunk);
        while let Some(event) = parser.next_event() {
            if out.send(protocol::event(id, &event)).await.is_err() {
                return false;
            }
        }
    }
    true
}
