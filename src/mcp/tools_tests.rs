use super::*;
use crate::mcp::types::{SocketRequest, SocketResponse, TabInfo};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_sock() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("opman-toolstest-{}-{}.sock", std::process::id(), n));
    let _ = std::fs::remove_file(&p);
    p
}

fn bad_socket() -> PathBuf {
    std::env::temp_dir().join("opman-tools-missing-sock.sock")
}

/// Spawn a mock socket server that replies to each request via `handler`.
fn spawn_mock<F>(handler: F) -> PathBuf
where
    F: Fn(SocketRequest) -> SocketResponse + Send + Sync + 'static,
{
    let path = unique_sock();
    let h = Arc::new(handler);
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let h = h.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut br = tokio::io::BufReader::new(reader);
                let mut buf = String::new();
                let _ = br.read_line(&mut buf).await;
                let resp = match serde_json::from_str::<SocketRequest>(buf.trim()) {
                    Ok(req) => h(req),
                    Err(e) => SocketResponse::err(format!("bad req: {e}")),
                };
                let line = serde_json::to_string(&resp).unwrap();
                let _ = writer.write_all(line.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                let _ = writer.shutdown().await;
            });
        }
    });
    path
}

/// Standard "everything works" flow; status reports running once, then success.
fn ok_flow() -> impl Fn(SocketRequest) -> SocketResponse {
    let n = Arc::new(AtomicUsize::new(0));
    move |req| match req.op.as_str() {
        "new" => SocketResponse::ok_tab_created(1),
        "list" => SocketResponse::ok_tabs(vec![TabInfo {
            index: 0,
            active: true,
            name: "main".into(),
        }]),
        "read" => SocketResponse::ok_text("cmd output".into()),
        "status" => {
            let c = n.fetch_add(1, Ordering::Relaxed);
            if c == 0 {
                SocketResponse::ok_status("running".into())
            } else {
                SocketResponse::ok_status("success".into())
            }
        }
        _ => SocketResponse::ok_empty(),
    }
}

// ── argument / dispatch error branches (no socket contact needed) ────────────

#[tokio::test]
async fn missing_params_errors() {
    let res = handle_tool_call(&bad_socket(), None, None).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Missing tool name"));
}

#[tokio::test]
async fn missing_name_errors() {
    let res = handle_tool_call(&bad_socket(), Some(json!({})), None).await;
    assert!(res.unwrap_err().to_string().contains("Missing tool name"));
}

#[tokio::test]
async fn unknown_tool_returns_text() {
    let res = handle_tool_call(&bad_socket(), Some(json!({"name":"nope"})), None)
        .await
        .unwrap();
    assert!(res[0]["text"]
        .as_str()
        .unwrap()
        .contains("Unknown tool: nope"));
}

#[tokio::test]
async fn terminal_run_missing_command() {
    let res = handle_tool_call(&bad_socket(), Some(json!({"name":"terminal_run"})), None).await;
    assert!(res.unwrap_err().to_string().contains("requires 'command'"));
}

#[tokio::test]
async fn terminal_rename_missing_tab() {
    let res = handle_tool_call(
        &bad_socket(),
        Some(json!({"name":"terminal_rename","arguments":{"name":"x"}})),
        None,
    )
    .await;
    assert!(res.unwrap_err().to_string().contains("requires 'tab'"));
}

#[tokio::test]
async fn terminal_rename_missing_name() {
    let res = handle_tool_call(
        &bad_socket(),
        Some(json!({"name":"terminal_rename","arguments":{"tab":1}})),
        None,
    )
    .await;
    assert!(res.unwrap_err().to_string().contains("requires 'name'"));
}

#[tokio::test]
async fn ephemeral_missing_command() {
    let res = handle_tool_call(
        &bad_socket(),
        Some(json!({"name":"terminal_ephemeral_run","arguments":{"name":"build"}})),
        None,
    )
    .await;
    assert!(res.unwrap_err().to_string().contains("requires 'command'"));
}

#[tokio::test]
async fn ephemeral_missing_name() {
    let res = handle_tool_call(
        &bad_socket(),
        Some(json!({"name":"terminal_ephemeral_run","arguments":{"command":"ls"}})),
        None,
    )
    .await;
    assert!(res.unwrap_err().to_string().contains("requires 'name'"));
}

#[tokio::test]
async fn terminal_read_connect_error() {
    // Reaches send_socket_request against a missing socket → Err.
    let res = handle_tool_call(&bad_socket(), Some(json!({"name":"terminal_read"})), None).await;
    assert!(res.is_err());
}

// ── happy paths against the mock ─────────────────────────────────────────────

#[tokio::test]
async fn terminal_read_ok() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_read","arguments":{"tab":0,"last_n":5}})),
        Some("sid"),
    )
    .await
    .unwrap();
    assert_eq!(res[0]["text"], "cmd output");
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_list_ok() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(&sock, Some(json!({"name":"terminal_list"})), None)
        .await
        .unwrap();
    assert!(res[0]["text"].as_str().unwrap().contains("Tab 0"));
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_new_ok() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_new","arguments":{"name":"work"}})),
        None,
    )
    .await
    .unwrap();
    assert!(res[0]["text"].as_str().unwrap().contains("index 1"));
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_close_ok() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_close","arguments":{"tab":2}})),
        None,
    )
    .await
    .unwrap();
    assert_eq!(res[0]["text"], "OK");
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_rename_ok() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_rename","arguments":{"tab":1,"name":"renamed"}})),
        None,
    )
    .await
    .unwrap();
    assert_eq!(res[0]["text"], "OK");
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_run_no_wait() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_run","arguments":{"command":"ls","tab":0}})),
        None,
    )
    .await
    .unwrap();
    assert_eq!(res[0]["text"], "OK");
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_run_wait_success() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_run","arguments":{"command":"ls","tab":0,"wait":true,"timeout":5}})),
        None,
    )
    .await
    .unwrap();
    assert_eq!(res[0]["text"], "cmd output");
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_run_wait_but_run_not_ok() {
    // run returns an error → poll is skipped, error text is returned.
    let sock = spawn_mock(|req: SocketRequest| {
        if req.op == "run" {
            SocketResponse::err("run rejected".into())
        } else {
            SocketResponse::ok_empty()
        }
    });
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_run","arguments":{"command":"x","tab":0,"wait":true}})),
        None,
    )
    .await
    .unwrap();
    assert!(res[0]["text"].as_str().unwrap().contains("run rejected"));
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn terminal_run_wait_timeout() {
    // status never leaves "running" → poll times out → [TIMEOUT ...] prefix.
    let sock = spawn_mock(|req: SocketRequest| match req.op.as_str() {
        "read" => SocketResponse::ok_text("partial".into()),
        "status" => SocketResponse::ok_status("running".into()),
        _ => SocketResponse::ok_empty(),
    });
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_run","arguments":{"command":"x","tab":0,"wait":true,"timeout":1}})),
        None,
    )
    .await
    .unwrap();
    let text = res[0]["text"].as_str().unwrap();
    assert!(text.contains("[TIMEOUT after 1s]"));
    assert!(text.contains("partial"));
    let _ = std::fs::remove_file(&sock);
}

// ── ephemeral run composite flow ─────────────────────────────────────────────

#[tokio::test]
async fn ephemeral_full_flow_success() {
    let sock = spawn_mock(ok_flow());
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_ephemeral_run","arguments":{"command":"make","name":"build","timeout":5}})),
        Some("sid"),
    )
    .await
    .unwrap();
    assert_eq!(res[0]["text"], "cmd output");
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn ephemeral_lock_rejected() {
    let sock = spawn_mock(|req: SocketRequest| {
        if req.op == "ephemeral_lock" {
            SocketResponse::err("already running".into())
        } else {
            SocketResponse::ok_empty()
        }
    });
    let res = handle_tool_call(
        &sock,
        Some(
            json!({"name":"terminal_ephemeral_run","arguments":{"command":"make","name":"build"}}),
        ),
        None,
    )
    .await
    .unwrap();
    assert!(res[0]["text"].as_str().unwrap().contains("already running"));
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn ephemeral_new_fails() {
    let sock = spawn_mock(|req: SocketRequest| match req.op.as_str() {
        "new" => SocketResponse::err("cannot create tab".into()),
        _ => SocketResponse::ok_empty(),
    });
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_ephemeral_run","arguments":{"command":"make","name":"b"}})),
        None,
    )
    .await
    .unwrap();
    assert!(res[0]["text"]
        .as_str()
        .unwrap()
        .contains("cannot create tab"));
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn ephemeral_run_fails() {
    let sock = spawn_mock(|req: SocketRequest| match req.op.as_str() {
        "new" => SocketResponse::ok_tab_created(3),
        "run" => SocketResponse::err("run boom".into()),
        _ => SocketResponse::ok_empty(),
    });
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_ephemeral_run","arguments":{"command":"make","name":"b"}})),
        None,
    )
    .await
    .unwrap();
    assert!(res[0]["text"].as_str().unwrap().contains("run boom"));
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn ephemeral_timeout() {
    let sock = spawn_mock(|req: SocketRequest| match req.op.as_str() {
        "new" => SocketResponse::ok_tab_created(1),
        "read" => SocketResponse::ok_text("still going".into()),
        "status" => SocketResponse::ok_status("running".into()),
        _ => SocketResponse::ok_empty(),
    });
    let res = handle_tool_call(
        &sock,
        Some(json!({"name":"terminal_ephemeral_run","arguments":{"command":"x","name":"b","timeout":1}})),
        None,
    )
    .await
    .unwrap();
    let text = res[0]["text"].as_str().unwrap();
    assert!(text.contains("[TIMEOUT after 1s]"));
    let _ = std::fs::remove_file(&sock);
}
