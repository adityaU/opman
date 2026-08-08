//! Wave-3 breadth coverage for the kanban MCP `mod.rs`: the `tools/call` route driven
//! against a synchronous mock HTTP server for every tool + its argument-validation
//! branches (internal present and absent).
use super::*;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── mock internal HTTP server (bind synchronously before accepting) ───────────

struct Mock {
    url: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Mock {
    fn start(status: u16, body: &'static str) -> Mock {
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
        Mock {
            url: format!("http://127.0.0.1:{}", port),
            stop,
            handle: Some(handle),
        }
    }

    fn internal(&self) -> Internal {
        Internal {
            url: self.url.clone(),
            token: "tok".to_string(),
            client: reqwest::Client::new(),
        }
    }
}

async fn call(internal: &Internal, name: &str, args: serde_json::Value) -> String {
    let params = Some(json!({ "name": name, "arguments": args }));
    let v = route_request(Some(internal), "tools/call", params, json!(1))
        .await
        .unwrap();
    v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

// ── every tool: success path via mock ────────────────────────────────────────

#[tokio::test]
async fn get_task_success_body_passed_through() {
    let mock = Mock::start(200, r#"{"lane":"Doing"}"#);
    let out = call(
        &mock.internal(),
        "kanban_get_task",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert!(out.contains("Doing"), "got: {out}");
}

#[tokio::test]
async fn board_summary_success() {
    let mock = Mock::start(200, "BOARD-OK");
    let out = call(
        &mock.internal(),
        "kanban_board_summary",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert_eq!(out, "BOARD-OK");
}

#[tokio::test]
async fn set_lane_success() {
    let mock = Mock::start(200, "moved");
    let out = call(
        &mock.internal(),
        "kanban_set_lane",
        json!({ "task_id": "tsk_1", "lane": "Implementing" }),
    )
    .await;
    assert_eq!(out, "moved");
}

#[tokio::test]
async fn add_note_success() {
    let mock = Mock::start(200, "noted");
    let out = call(
        &mock.internal(),
        "kanban_add_note",
        json!({ "task_id": "tsk_1", "body": "progress!" }),
    )
    .await;
    assert_eq!(out, "noted");
}

#[tokio::test]
async fn complete_success_even_without_summary() {
    // summary is optional; missing → empty string, still posts.
    let mock = Mock::start(200, "completed");
    let out = call(
        &mock.internal(),
        "kanban_complete",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert_eq!(out, "completed");
}

#[tokio::test]
async fn list_tasks_success_with_filters() {
    let mock = Mock::start(200, "[]");
    let out = call(
        &mock.internal(),
        "kanban_list_tasks",
        json!({ "task_id": "tsk_1", "lane": "Doing", "tags": ["a"], "query": "x", "include_archived": true }),
    )
    .await;
    assert_eq!(out, "[]");
}

#[tokio::test]
async fn read_notes_success() {
    let mock = Mock::start(200, "{}");
    let out = call(
        &mock.internal(),
        "kanban_read_notes",
        json!({ "task_id": "tsk_1", "task_ids": ["tsk_2", "tsk_3"] }),
    )
    .await;
    assert_eq!(out, "{}");
}

// ── argument-validation branches (internal present, no network hit) ───────────

#[tokio::test]
async fn missing_task_id_short_circuits() {
    let mock = Mock::start(200, "unused");
    let out = call(&mock.internal(), "kanban_get_task", json!({})).await;
    assert!(
        out.contains("Missing required argument: task_id"),
        "got: {out}"
    );
}

#[tokio::test]
async fn set_lane_missing_lane() {
    let mock = Mock::start(200, "unused");
    let out = call(
        &mock.internal(),
        "kanban_set_lane",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert!(
        out.contains("Missing required argument: lane"),
        "got: {out}"
    );
}

#[tokio::test]
async fn add_note_missing_body() {
    let mock = Mock::start(200, "unused");
    let out = call(
        &mock.internal(),
        "kanban_add_note",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert!(
        out.contains("Missing required argument: body"),
        "got: {out}"
    );
}

#[tokio::test]
async fn unknown_tool_name_reported() {
    let mock = Mock::start(200, "unused");
    let out = call(
        &mock.internal(),
        "kanban_frobnicate",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert!(
        out.contains("Unknown tool: kanban_frobnicate"),
        "got: {out}"
    );
}

// ── error status mapping ─────────────────────────────────────────────────────

#[tokio::test]
async fn non_success_status_wrapped() {
    let mock = Mock::start(500, "boom");
    let out = call(
        &mock.internal(),
        "kanban_get_task",
        json!({ "task_id": "tsk_1" }),
    )
    .await;
    assert!(out.contains("Error 500: boom"), "got: {out}");
}

// ── internal = None path for a couple of tools ───────────────────────────────

#[tokio::test]
async fn no_internal_returns_unavailable_for_any_tool() {
    for name in ["kanban_get_task", "kanban_set_lane", "kanban_read_notes"] {
        let params = Some(json!({ "name": name, "arguments": { "task_id": "tsk_1" } }));
        let v = route_request(None, "tools/call", params, json!(2))
            .await
            .unwrap();
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Kanban API is unavailable"), "got: {text}");
    }
}

// ── request struct deserialization ───────────────────────────────────────────

#[test]
fn mcp_request_deserializes() {
    let r: McpRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"tools/list","id":9}"#).unwrap();
    assert_eq!(r.method, "tools/list");
    assert!(r.params.is_none());
    assert_eq!(r.id, json!(9));
}
