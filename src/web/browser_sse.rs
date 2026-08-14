//! Live frames for a browser pane that cannot use an iframe.
//!
//! Frames are pushed, not polled: the stream parks on the screencast's version counter
//! and wakes only when Chromium produced something new. A page that is sitting still
//! therefore costs one keep-alive every 15 seconds rather than a JPEG every tick.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;
use serde::Deserialize;

use super::auth::check_auth_manual;
use super::error::WebError;
use super::types::ServerState;

#[derive(Deserialize)]
pub struct FrameQuery {
    pane_id: String,
    /// `EventSource` cannot set headers, so the JWT rides on the query string — the same
    /// bargain the terminal stream makes.
    token: Option<String>,
}

/// GET /api/browser/stream?pane_id=…&token=…
pub async fn browser_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(query): Query<FrameQuery>,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &query.token) {
        return Err(WebError::Unauthorized);
    }

    let pane = state
        .browser
        .get(&query.pane_id)
        .await
        .ok_or(WebError::NotFound("browser pane is not open"))?;

    let stream = async_stream::stream! {
        // Holding the viewer for the life of the stream is what starts the screencast and,
        // on disconnect, stops it — no explicit teardown to forget.
        let screencast = pane.tab().screencast().clone();
        let _viewer = screencast.viewer().await;

        let mut seen = 0_u64;
        // Lead with whatever is already rendered so a reconnecting pane is not blank until
        // the page next changes.
        if let Some((frame, version)) = screencast.latest().await {
            seen = version;
            yield Ok::<_, Infallible>(SseEvent::default().event("frame").data(frame.as_ref()));
        }

        loop {
            let Some((frame, version)) = screencast.next_after(seen).await else {
                continue;
            };
            seen = version;
            yield Ok::<_, Infallible>(SseEvent::default().event("frame").data(frame.as_ref()));
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
