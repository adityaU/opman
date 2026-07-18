//! Generated tests for the SSE listener module: `extract_sse_data`, struct
//! deserialization, and `run_sse_stream` driven against a mock HTTP server.
//!
//! `connect_sse` uses the process-global `crate::app::base_url()` (a dead port
//! in tests) so its success arm can't be reached in a unit test; we drive its
//! reconnect error arm with a timeout. `run_sse_stream` takes `base_url` as a
//! parameter, so we point it at a locally-bound mock listener to cover the
//! success (streaming) and non-2xx (bail) paths.

use super::*;
use crate::app::BackgroundEvent;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

// ── extract_sse_data ────────────────────────────────────────────────

#[test]
fn extract_single_data_line() {
    let msg = "event: message\ndata: hello world";
    assert_eq!(extract_sse_data(msg), Some("hello world".to_string()));
}

#[test]
fn extract_joins_multiple_data_lines() {
    let msg = "data: line1\ndata: line2";
    assert_eq!(extract_sse_data(msg), Some("line1\nline2".to_string()));
}

#[test]
fn extract_trims_whitespace_after_prefix() {
    let msg = "data:    spaced   ";
    assert_eq!(extract_sse_data(msg), Some("spaced".to_string()));
}

#[test]
fn extract_returns_none_when_no_data_lines() {
    let msg = "event: ping\nid: 5\n: comment";
    assert_eq!(extract_sse_data(msg), None);
}

#[test]
fn extract_returns_none_for_empty_message() {
    assert_eq!(extract_sse_data(""), None);
}

#[test]
fn extract_ignores_non_data_prefix_lines() {
    let msg = "retry: 100\ndata: payload\nfoo: bar";
    assert_eq!(extract_sse_data(msg), Some("payload".to_string()));
}

// ── struct deserialization sanity ───────────────────────────────────

#[test]
fn sse_event_deserializes_with_rename() {
    let ev: SseEvent =
        serde_json::from_str(r#"{"type":"session.created","properties":{"a":1}}"#).unwrap();
    assert_eq!(ev.event_type, "session.created");
    assert_eq!(ev.properties["a"], 1);
}

#[test]
fn session_status_props_deserializes() {
    let p: SessionStatusProps =
        serde_json::from_value(serde_json::json!({"sessionID":"s","status":{"type":"busy"}}))
            .unwrap();
    assert_eq!(p.session_id, "s");
    assert_eq!(p.status.status_type, "busy");
}

#[test]
fn message_updated_props_defaults() {
    // Only sessionID is required; tokens/cost default to zero.
    let p: MessageUpdatedProps =
        serde_json::from_value(serde_json::json!({"info":{"sessionID":"s"}})).unwrap();
    assert_eq!(p.info.session_id, "s");
    assert_eq!(p.info.cost, 0.0);
    assert_eq!(p.info.tokens.input, 0);
    assert_eq!(p.info.tokens.cache.read, 0);
}

// ── run_sse_stream against a mock HTTP server ───────────────────────

/// Bind a listener synchronously, then answer exactly one request with the
/// given raw HTTP response, then close. Returns the base URL to hand to
/// `run_sse_stream`.
async fn mock_server(response: &'static str) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            // Drain the request headers so the client's write completes.
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf).await;
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.flush().await;
            // Give the client time to read before the socket drops.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn run_sse_stream_success_dispatches_events() {
    let body = "data: {\"type\":\"session.created\",\"properties\":{\"info\":{\"id\":\"srv1\",\"directory\":\"/p\"}}}\n\n\
                data: {\"type\":\"server.connected\",\"properties\":{}}\n\n";
    // Leak a String so we can hand a &'static str to the mock task.
    let response: &'static str = Box::leak(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .into_boxed_str(),
    );
    let (base_url, server) = mock_server(response).await;

    let (tx, mut rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let res = tokio::time::timeout(
        Duration::from_millis(2000),
        run_sse_stream(&tx, 0, &base_url, "/p"),
    )
    .await
    .expect("run_sse_stream should complete once body is consumed");
    assert!(res.is_ok());

    let mut got_created = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, BackgroundEvent::SseSessionCreated { session, .. } if session.id == "srv1") {
            got_created = true;
        }
    }
    assert!(got_created, "expected a SseSessionCreated from the streamed event");
    let _ = server.await;
}

#[tokio::test]
async fn run_sse_stream_non_success_bails() {
    let (base_url, server) =
        mock_server("HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n").await;
    let (tx, _rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let res = tokio::time::timeout(
        Duration::from_millis(2000),
        run_sse_stream(&tx, 0, &base_url, "/p"),
    )
    .await
    .expect("should return quickly");
    assert!(res.is_err(), "non-2xx status must bail with an error");
    let _ = server.await;
}

#[tokio::test]
async fn run_sse_stream_connection_refused_errors() {
    // Nothing listens on this port → connect error → Err.
    let (tx, _rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let res = run_sse_stream(&tx, 0, "http://127.0.0.1:1", "/p").await;
    assert!(res.is_err());
}

// ── connect_sse reconnect (error) arm + spawn_sse_listener ──────────

#[tokio::test]
async fn connect_sse_error_arm_then_would_sleep() {
    // base_url() is a dead port in tests → run_sse_stream errors fast → the
    // reconnect (warn + 3s sleep) arm runs; we cut it off with a timeout.
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1".to_string());
    let (tx, _rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let r = tokio::time::timeout(
        Duration::from_millis(300),
        connect_sse(tx, 0, "/tmp/proj".to_string()),
    )
    .await;
    // connect_sse loops forever, so the timeout must elapse.
    assert!(r.is_err());
}

#[tokio::test]
async fn spawn_sse_listener_does_not_panic() {
    let (tx, _rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    spawn_sse_listener(&tx, 0, "/tmp/proj".to_string());
    // The spawned task connects to a dead port and enters its reconnect loop;
    // we only assert the spawn call is well-formed.
}
