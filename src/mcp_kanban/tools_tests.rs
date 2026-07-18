use super::*;
use serde_json::json;

/// Minimal one-shot HTTP server on a background OS thread. Serves exactly one
/// request with the given status + body, then exits. Returns the base URL.
fn spawn_http(status: u16, body: &str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = body.to_string();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let reason = if (200..300).contains(&status) {
                "OK"
            } else {
                "ERR"
            };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn internal(url: &str) -> Internal {
    Internal {
        url: url.to_string(),
        token: "test-token".to_string(),
        client: reqwest::Client::new(),
    }
}

// ── argument validation (no request issued) ──────────────────────────────────

#[tokio::test]
async fn unavailable_without_internal() {
    let out = dispatch_tool(None, Some(json!({"name":"kanban_get_task"}))).await;
    assert!(out.contains("Kanban API is unavailable"));
}

#[tokio::test]
async fn missing_task_id() {
    let i = internal("http://127.0.0.1:1");
    let out = dispatch_tool(Some(&i), Some(json!({"name":"kanban_get_task","arguments":{}}))).await;
    assert!(out.contains("Missing required argument: task_id"));
}

#[tokio::test]
async fn set_lane_missing_lane() {
    let i = internal("http://127.0.0.1:1");
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_set_lane","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("Missing required argument: lane"));
}

#[tokio::test]
async fn add_note_missing_body() {
    let i = internal("http://127.0.0.1:1");
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_add_note","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("Missing required argument: body"));
}

#[tokio::test]
async fn unknown_tool() {
    let i = internal("http://127.0.0.1:1");
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_bogus","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("Unknown tool: kanban_bogus"));
}

#[tokio::test]
async fn none_params_missing_task_id() {
    let i = internal("http://127.0.0.1:1");
    let out = dispatch_tool(Some(&i), None).await;
    assert!(out.contains("Missing required argument: task_id"));
}

// ── successful GET / POST via mock ───────────────────────────────────────────

#[tokio::test]
async fn get_task_success() {
    let url = spawn_http(200, r#"{"title":"Do the thing"}"#);
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_get_task","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("Do the thing"));
}

#[tokio::test]
async fn set_lane_success() {
    let url = spawn_http(200, r#"{"ok":true}"#);
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_set_lane","arguments":{"task_id":"tsk_1","lane":"Doing"}})),
    )
    .await;
    assert!(out.contains("ok"));
}

#[tokio::test]
async fn add_note_success() {
    let url = spawn_http(200, r#"{"noted":1}"#);
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_add_note","arguments":{"task_id":"tsk_1","body":"progress"}})),
    )
    .await;
    assert!(out.contains("noted"));
}

#[tokio::test]
async fn complete_success_empty_summary_allowed() {
    let url = spawn_http(200, r#"{"done":true}"#);
    let i = internal(&url);
    // summary omitted → empty string is allowed by kanban_complete.
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_complete","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("done"));
}

#[tokio::test]
async fn list_tasks_success() {
    let url = spawn_http(200, r#"[{"id":"tsk_2"}]"#);
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({
            "name":"kanban_list_tasks",
            "arguments":{"task_id":"tsk_1","lane":"Doing","tags":["a"],"query":"x","include_archived":true}
        })),
    )
    .await;
    assert!(out.contains("tsk_2"));
}

#[tokio::test]
async fn board_summary_success() {
    let url = spawn_http(200, r#"{"lanes":[]}"#);
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_board_summary","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("lanes"));
}

#[tokio::test]
async fn read_notes_success() {
    let url = spawn_http(200, r#"{"notes":[]}"#);
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_read_notes","arguments":{"task_id":"tsk_1","task_ids":["tsk_9"]}})),
    )
    .await;
    assert!(out.contains("notes"));
}

// ── error handling ───────────────────────────────────────────────────────────

#[tokio::test]
async fn non_success_status_is_reported() {
    let url = spawn_http(500, "boom");
    let i = internal(&url);
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_get_task","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("Error 500"));
    assert!(out.contains("boom"));
}

#[tokio::test]
async fn request_failure_is_reported() {
    // Port 1 → connection refused.
    let i = internal("http://127.0.0.1:1");
    let out = dispatch_tool(
        Some(&i),
        Some(json!({"name":"kanban_board_summary","arguments":{"task_id":"tsk_1"}})),
    )
    .await;
    assert!(out.contains("Request failed"));
}
