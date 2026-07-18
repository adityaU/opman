//! Wave-2 coverage: stdio read-loop, `load_internal_from`, and the
//! `tools/call` route against a synchronous mock HTTP server.
use super::*;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── mock internal HTTP server ────────────────────────────────────────────────

struct MockHttp {
    url: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MockHttp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl MockHttp {
    /// Bind synchronously (before spawning the accept thread) so callers never
    /// race the listener coming up.
    fn start(status: u16, body: &str) -> MockHttp {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let resp = format!(
            "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            body.len(),
            body
        );
        let handle = std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut s, _)) => {
                        s.set_nonblocking(false).ok();
                        s.set_read_timeout(Some(Duration::from_millis(500))).ok();
                        let mut buf = [0u8; 2048];
                        let _ = s.read(&mut buf);
                        let _ = s.write_all(resp.as_bytes());
                        let _ = s.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        MockHttp {
            url: format!("http://127.0.0.1:{}", port),
            stop,
            handle: Some(handle),
        }
    }

    fn internal(&self) -> Internal {
        Internal {
            url: self.url.clone(),
            token: "test-token".to_string(),
            client: reqwest::Client::new(),
        }
    }
}

// ── load_internal_from ───────────────────────────────────────────────────────

#[test]
fn load_internal_from_valid() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = dir.path().join("internal.json");
    std::fs::write(&p, r#"{"url":"http://127.0.0.1:9","token":"abc"}"#).unwrap();
    let got = load_internal_from(&p).unwrap();
    assert_eq!(got.url, "http://127.0.0.1:9");
    assert_eq!(got.token, "abc");
}

#[test]
fn load_internal_from_missing_file() {
    let p = std::path::Path::new("/tmp/opman-kanban-nonexistent-xyz.json");
    assert!(load_internal_from(p).is_none());
}

#[test]
fn load_internal_from_malformed_json() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = dir.path().join("internal.json");
    std::fs::write(&p, "{ not json").unwrap();
    assert!(load_internal_from(&p).is_none());
}

#[test]
fn load_internal_from_missing_url_field() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = dir.path().join("internal.json");
    std::fs::write(&p, r#"{"token":"abc"}"#).unwrap();
    assert!(load_internal_from(&p).is_none());
}

#[test]
fn load_internal_from_url_not_string() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = dir.path().join("internal.json");
    std::fs::write(&p, r#"{"url":123,"token":"abc"}"#).unwrap();
    assert!(load_internal_from(&p).is_none());
}

// ── tools/call route with a live (mocked) internal API ───────────────────────

#[tokio::test]
async fn route_tools_call_get_success() {
    let mock = MockHttp::start(200, r#"{"task":"tsk_1","lane":"Doing"}"#);
    let internal = mock.internal();
    let params = Some(json!({"name":"kanban_get_task","arguments":{"task_id":"tsk_1"}}));
    let v = route_request(Some(&internal), "tools/call", params, json!(5))
        .await
        .unwrap();
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Doing"), "got: {text}");
}

#[tokio::test]
async fn route_tools_call_post_set_lane_success() {
    let mock = MockHttp::start(200, "moved");
    let internal = mock.internal();
    let params = Some(json!({
        "name":"kanban_set_lane",
        "arguments":{"task_id":"tsk_1","lane":"Implementing"}
    }));
    let v = route_request(Some(&internal), "tools/call", params, json!(6))
        .await
        .unwrap();
    assert_eq!(v["result"]["content"][0]["text"], "moved");
}

#[tokio::test]
async fn route_tools_call_error_status() {
    let mock = MockHttp::start(404, "no such task");
    let internal = mock.internal();
    let params = Some(json!({"name":"kanban_board_summary","arguments":{"task_id":"tsk_x"}}));
    let v = route_request(Some(&internal), "tools/call", params, json!(7))
        .await
        .unwrap();
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Error 404"), "got: {text}");
}

// ── read-loop ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn run_kanban_bridge_drives_loop() {
    let input = concat!(
        "\n",
        "{oops\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"id\":1}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"id\":0}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\",\"id\":2}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"kanban_get_task\",\"arguments\":{\"task_id\":\"tsk_1\"}},\"id\":3}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"huh\",\"id\":4}\n",
    );
    let mut out: Vec<u8> = Vec::new();
    // internal = None → tools/call returns the "unavailable" text without network.
    run_kanban_bridge(None, input.as_bytes(), &mut out)
        .await
        .unwrap();
    let s = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 5, "got: {s}");
    assert!(s.contains("Parse error"));
    assert!(s.contains("opman-kanban"));
    assert!(s.contains("Kanban API is unavailable"));
    assert!(s.contains("Method not found: huh"));
}

#[tokio::test]
async fn run_kanban_bridge_eof() {
    let mut out: Vec<u8> = Vec::new();
    run_kanban_bridge(None, &b""[..], &mut out).await.unwrap();
    assert!(out.is_empty());
}
