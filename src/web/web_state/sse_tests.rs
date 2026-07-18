use super::*;
use crate::web::web_state::WebStateHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ── extract_sse_data ────────────────────────────────────────────────

#[test]
fn extract_sse_data_single_line() {
    let msg = "event: message\ndata: hello";
    assert_eq!(extract_sse_data(msg).as_deref(), Some("hello"));
}

#[test]
fn extract_sse_data_multiple_lines_joined() {
    let msg = "data: line1\ndata: line2";
    assert_eq!(extract_sse_data(msg).as_deref(), Some("line1\nline2"));
}

#[test]
fn extract_sse_data_none_when_no_data_field() {
    let msg = "event: ping\nid: 1";
    assert!(extract_sse_data(msg).is_none());
}

#[test]
fn extract_sse_data_none_when_empty() {
    assert!(extract_sse_data("").is_none());
}

// ── run_opencode_sse ────────────────────────────────────────────────

#[tokio::test]
async fn run_opencode_sse_connection_refused_errors() {
    let h = WebStateHandle::new_test();
    // Port 9 (discard) is closed → connect fails fast.
    let err = run_opencode_sse(&h, "http://127.0.0.1:9", "/dir").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn run_opencode_sse_non_success_status_errors() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf).await;
            let resp = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.flush().await;
        }
    });
    let h = WebStateHandle::new_test();
    let base = format!("http://{}", addr);
    let err = run_opencode_sse(&h, &base, "/dir").await.unwrap_err();
    assert!(err.to_string().contains("status"), "got: {err}");
}

#[tokio::test]
async fn run_opencode_sse_streams_event_then_ends() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf).await;
            // 200 OK event-stream with one complete SSE message, then close.
            let body = "data: {\"type\":\"noop\",\"properties\":{}}\n\n";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.flush().await;
            // Drop the socket to end the stream cleanly.
        }
    });
    let h = WebStateHandle::new_test();
    let mut raw_rx = h.raw_sse_tx.subscribe();
    let base = format!("http://{}", addr);
    let res = run_opencode_sse(&h, &base, "/dir").await;
    assert!(res.is_ok(), "expected clean end, got {res:?}");
    // The extracted event data was re-broadcast to web clients.
    let got = raw_rx.try_recv();
    assert!(got.is_ok(), "expected rebroadcast data");
    assert!(got.unwrap().contains("noop"));
}
