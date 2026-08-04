//! Wave-2 coverage for the `event_stream` SSE handler: drive the stream with a
//! short timeout, asserting the initial `server.connected` frame and a
//! subsequently-broadcast, directory-matched event both arrive.

use super::*;
use crate::claude_p_engine::ClaudePEngine;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::IntoResponse;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

fn headers(dir: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    if !dir.is_empty() {
        h.insert("x-opencode-directory", HeaderValue::from_str(dir).unwrap());
    }
    h
}

async fn next_frame(body: &mut axum::body::BodyDataStream) -> String {
    let chunk = tokio::time::timeout(Duration::from_millis(200), body.next())
        .await
        .expect("frame within 200ms")
        .expect("stream not ended")
        .expect("frame ok");
    String::from_utf8_lossy(&chunk).to_string()
}

#[tokio::test]
async fn event_stream_emits_connected_then_matched_event() {
    let e = engine();
    let resp = event_stream(State(e.clone()), headers("/proj"))
        .await
        .into_response();
    let mut body = resp.into_body().into_data_stream();

    // First frame is always the synthetic server.connected event.
    let first = next_frame(&mut body).await;
    assert!(first.contains("server.connected"), "got: {first}");

    // A same-directory event is forwarded.
    e.emit(
        "/proj",
        "message.updated",
        serde_json::json!({ "info": { "id": "m1" } }),
    );
    let second = next_frame(&mut body).await;
    assert!(second.contains("message.updated"), "got: {second}");
}

#[tokio::test]
async fn event_stream_empty_dir_receives_all_events() {
    let e = engine();
    // No directory header → dir is empty → every event passes the filter.
    let resp = event_stream(State(e.clone()), headers(""))
        .await
        .into_response();
    let mut body = resp.into_body().into_data_stream();
    let _connected = next_frame(&mut body).await;

    e.emit(
        "/whatever",
        "session.status",
        serde_json::json!({ "sessionID": "x" }),
    );
    let frame = next_frame(&mut body).await;
    assert!(frame.contains("session.status"), "got: {frame}");
}

#[tokio::test]
async fn event_stream_filters_out_other_dir_events() {
    let e = engine();
    let resp = event_stream(State(e.clone()), headers("/proj"))
        .await
        .into_response();
    let mut body = resp.into_body().into_data_stream();
    let _connected = next_frame(&mut body).await;

    // An event for a different, non-empty directory is dropped by the filter,
    // so no frame arrives within the timeout.
    e.emit(
        "/other",
        "message.updated",
        serde_json::json!({ "info": { "id": "m2" } }),
    );
    let res = tokio::time::timeout(Duration::from_millis(150), body.next()).await;
    assert!(res.is_err(), "mismatched-dir event must be filtered out");
}
