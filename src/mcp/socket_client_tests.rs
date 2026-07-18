use super::*;
use crate::mcp::types::{SocketRequest, SocketResponse, TabInfo};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_sock() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("opman-sctest-{}-{}.sock", std::process::id(), n));
    let _ = std::fs::remove_file(&p);
    p
}

/// Serve `raw_lines`: each accepted connection reads one request line and
/// replies with the next raw line (verbatim + newline).
fn spawn_raw_server(lines: Vec<String>) -> PathBuf {
    let path = unique_sock();
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        for line in lines {
            let (stream, _) = match listener.accept().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let (reader, mut writer) = stream.into_split();
            let mut br = tokio::io::BufReader::new(reader);
            let mut buf = String::new();
            let _ = tokio::io::AsyncBufReadExt::read_line(&mut br, &mut buf).await;
            use tokio::io::AsyncWriteExt;
            let _ = writer.write_all(line.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.shutdown().await;
        }
    });
    path
}

fn spawn_server(responses: Vec<SocketResponse>) -> PathBuf {
    spawn_raw_server(
        responses
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect(),
    )
}

/// Serve the same response forever (for timeout tests).
fn spawn_repeating(resp: SocketResponse) -> PathBuf {
    let path = unique_sock();
    let line = serde_json::to_string(&resp).unwrap();
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let l = line.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut br = tokio::io::BufReader::new(reader);
                let mut buf = String::new();
                let _ = tokio::io::AsyncBufReadExt::read_line(&mut br, &mut buf).await;
                use tokio::io::AsyncWriteExt;
                let _ = writer.write_all(l.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                let _ = writer.shutdown().await;
            });
        }
    });
    path
}

// ── format_mcp_response ──────────────────────────────────────────────────────

#[test]
fn format_error_response() {
    let r = SocketResponse::err("nope".into());
    let v = format_mcp_response(&r).unwrap();
    assert_eq!(v[0]["type"], "text");
    assert_eq!(v[0]["text"], "nope");
}

#[test]
fn format_error_response_without_message() {
    let r = SocketResponse {
        ok: false,
        output: None,
        tabs: None,
        error: None,
        tab_index: None,
        command_state: None,
    };
    let v = format_mcp_response(&r).unwrap();
    assert_eq!(v[0]["text"], "Unknown error");
}

#[test]
fn format_output_response() {
    let r = SocketResponse::ok_text("stdout here".into());
    let v = format_mcp_response(&r).unwrap();
    assert_eq!(v[0]["text"], "stdout here");
}

#[test]
fn format_tabs_response_named_and_unnamed() {
    let r = SocketResponse::ok_tabs(vec![
        TabInfo {
            index: 0,
            active: true,
            name: "build".into(),
        },
        TabInfo {
            index: 1,
            active: false,
            name: "".into(),
        },
    ]);
    let v = format_mcp_response(&r).unwrap();
    let text = v[0]["text"].as_str().unwrap();
    assert!(text.contains("Tab 0 \"build\" (active)"));
    assert!(text.contains("Tab 1"));
    assert!(!text.contains("Tab 1 \""));
}

#[test]
fn format_tab_created_response() {
    let r = SocketResponse::ok_tab_created(4);
    let v = format_mcp_response(&r).unwrap();
    assert!(v[0]["text"].as_str().unwrap().contains("index 4"));
}

#[test]
fn format_empty_ok_response() {
    let r = SocketResponse::ok_empty();
    let v = format_mcp_response(&r).unwrap();
    assert_eq!(v[0]["text"], "OK");
}

// ── send_socket_request ──────────────────────────────────────────────────────

#[tokio::test]
async fn send_request_connect_error() {
    let bad = std::env::temp_dir().join("opman-does-not-exist-xyz.sock");
    let _ = std::fs::remove_file(&bad);
    let req = SocketRequest {
        op: "list".into(),
        ..Default::default()
    };
    let res = send_socket_request(&bad, &req).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Failed to connect"));
}

#[tokio::test]
async fn send_request_success() {
    let path = spawn_server(vec![SocketResponse::ok_text("pong".into())]);
    let req = SocketRequest {
        op: "read".into(),
        ..Default::default()
    };
    let resp = send_socket_request(&path, &req).await.unwrap();
    assert!(resp.ok);
    assert_eq!(resp.output.as_deref(), Some("pong"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn send_request_invalid_response() {
    let path = spawn_raw_server(vec!["this is not json".into()]);
    let req = SocketRequest {
        op: "read".into(),
        ..Default::default()
    };
    let res = send_socket_request(&path, &req).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Invalid response"));
    let _ = std::fs::remove_file(&path);
}

// ── close_tab ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn close_tab_success() {
    let path = spawn_server(vec![SocketResponse::ok_empty()]);
    let resp = close_tab(&path, 2, Some("sid")).await.unwrap();
    assert!(resp.ok);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn close_tab_error() {
    let bad = std::env::temp_dir().join("opman-close-missing.sock");
    let _ = std::fs::remove_file(&bad);
    assert!(close_tab(&bad, 0, None).await.is_err());
}

// ── poll_command_completion ──────────────────────────────────────────────────

#[tokio::test]
async fn poll_returns_true_on_immediate_timeout() {
    let bad = std::env::temp_dir().join("opman-poll-timeout0.sock");
    // timeout 0 → deadline already reached before first send.
    let timed_out = poll_command_completion(&bad, Some(0), 0, None).await;
    assert!(timed_out);
}

#[tokio::test]
async fn poll_returns_false_when_send_fails() {
    let bad = std::env::temp_dir().join("opman-poll-badsock.sock");
    let _ = std::fs::remove_file(&bad);
    // Non-zero timeout so it reaches the send, which fails → returns false.
    let timed_out = poll_command_completion(&bad, None, 5, None).await;
    assert!(!timed_out);
}

#[tokio::test]
async fn poll_completes_running_then_done() {
    // Phase 1 sees "running", phase 2 sees non-running → completed (false).
    let path = spawn_server(vec![
        SocketResponse::ok_status("running".into()),
        SocketResponse::ok_status("success".into()),
    ]);
    let timed_out = poll_command_completion(&path, Some(0), 5, Some("s")).await;
    assert!(!timed_out);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn poll_times_out_while_still_running() {
    // Always "running" → phase 2 loops until the deadline → returns true.
    let path = spawn_repeating(SocketResponse::ok_status("running".into()));
    let timed_out = poll_command_completion(&path, Some(1), 1, None).await;
    assert!(timed_out);
    let _ = std::fs::remove_file(&path);
}
