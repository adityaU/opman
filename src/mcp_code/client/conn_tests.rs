use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;
use crate::mcp_code::testing::{connect, spawn_fake};

const IDLE: Duration = Duration::from_secs(5);

fn methods(seen: &crate::mcp_code::testing::Seen) -> Vec<String> {
    seen.lock()
        .expect("lock")
        .iter()
        .map(|m| m["method"].as_str().unwrap_or("?").to_string())
        .collect()
}

#[tokio::test]
async fn handshake_is_initialize_then_a_reply_free_initialized() {
    let (conn, seen) = connect().await;
    conn.request("tools/list", json!({}), IDLE)
        .await
        .expect("list");
    conn.request("tools/call", json!({"name": "echo", "arguments": {}}), IDLE)
        .await
        .expect("call");
    assert_eq!(
        methods(&seen),
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/call"
        ]
    );
    let all = seen.lock().expect("lock");
    assert_eq!(all[0]["id"], 1);
    assert_eq!(all[0]["params"]["clientInfo"]["name"], "opman-code");
    // A notification carries no id, so nothing can be answered.
    assert!(all[1].get("id").is_none());
    // Calls carry a progress token so servers can keep the idle clock alive.
    assert_eq!(all[3]["params"]["_meta"]["progressToken"], all[3]["id"]);
}

#[tokio::test]
async fn calls_run_side_by_side() {
    let (conn, _) = connect().await;
    let started = Instant::now();
    let call = |ms: u64| {
        conn.request(
            "tools/call",
            json!({"name": "sleep", "arguments": {"ms": ms}}),
            IDLE,
        )
    };
    let (a, b, c) = tokio::join!(call(250), call(250), call(250));
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn rpc_errors_carry_code_and_message() {
    let (conn, _) = connect().await;
    let err = conn
        .request("bogus", json!({}), IDLE)
        .await
        .expect_err("error");
    match err {
        CallError::Rpc { code, message } => {
            assert_eq!(code, -32601);
            assert_eq!(message, "nope");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn silence_past_the_idle_window_gives_up() {
    let (conn, _) = connect().await;
    let err = conn
        .request(
            "tools/call",
            json!({"name": "sleep", "arguments": {"ms": 3000}}),
            Duration::from_millis(100),
        )
        .await
        .expect_err("idle");
    assert!(matches!(err, CallError::Idle(_)), "{err:?}");
}

#[tokio::test]
async fn giving_up_tells_the_server_to_cancel() {
    let (conn, seen) = connect().await;
    let slow = conn.request(
        "tools/call",
        json!({"name": "sleep", "arguments": {"ms": 3000}}),
        IDLE,
    );
    let _ = tokio::time::timeout(Duration::from_millis(100), slow).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let cancelled = seen
        .lock()
        .expect("lock")
        .iter()
        .find(|m| m["method"] == "notifications/cancelled")
        .cloned()
        .expect("a cancel notification");
    assert_eq!(cancelled["params"]["requestId"], 2);
}

#[tokio::test]
async fn a_closed_pipe_fails_open() {
    let (reader, writer, _) = spawn_fake();
    drop(writer);
    let result = Connection::open(reader, tokio::io::sink(), None).await;
    assert!(
        matches!(result, Err(CallError::Closed)),
        "{:?}",
        result.err()
    );
}

/// A server that handles `initialize`, then answers one `tools/call` after progress ticks.
async fn progress_server(
    extra: Vec<Value>,
    ticks: u64,
    tick_ms: u64,
) -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    let (client_out, server_in) = tokio::io::duplex(1 << 16);
    let (mut server_out, client_in) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        let mut lines = BufReader::new(server_in).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let message: Value = serde_json::from_str(&line).unwrap_or_default();
            let Some(id) = message.get("id").cloned() else {
                continue;
            };
            let token = message.pointer("/params/_meta/progressToken").cloned();
            if message["method"] == "tools/call" {
                for step in 0..ticks {
                    tokio::time::sleep(Duration::from_millis(tick_ms)).await;
                    let note = json!({"jsonrpc": "2.0", "method": "notifications/progress",
                        "params": {"progressToken": token, "progress": step + 1}});
                    let _ = server_out.write_all(format!("{note}\n").as_bytes()).await;
                }
            }
            for note in &extra {
                let _ = server_out.write_all(format!("{note}\n").as_bytes()).await;
            }
            let reply = json!({"jsonrpc": "2.0", "id": id, "result": {"content": []}});
            let _ = server_out.write_all(format!("{reply}\n").as_bytes()).await;
        }
    });
    (client_in, client_out)
}

#[tokio::test]
async fn progress_restarts_the_idle_clock() {
    // 6 ticks of 60ms is far past the 150ms window, but no gap exceeds it.
    let (reader, writer) = progress_server(vec![], 6, 60).await;
    let conn = Connection::open(reader, writer, None).await.expect("open");
    let result = conn
        .request(
            "tools/call",
            json!({"name": "x"}),
            Duration::from_millis(150),
        )
        .await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn list_changed_marks_the_connection_stale_once() {
    let note = json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"});
    let (reader, writer) = progress_server(vec![note], 0, 0).await;
    let conn = Connection::open(reader, writer, None).await.expect("open");
    assert!(
        conn.take_stale(),
        "initialize reply is preceded by the notification"
    );
    assert!(!conn.take_stale());
}

#[tokio::test]
async fn server_requests_are_refused_not_left_hanging() {
    let ask = json!({"jsonrpc": "2.0", "id": 99, "method": "roots/list"});
    let (client_out, server_in) = tokio::io::duplex(1 << 16);
    let (mut server_out, client_in) = tokio::io::duplex(1 << 16);
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut lines = BufReader::new(server_in).lines();
        let first = lines.next_line().await.ok().flatten().unwrap_or_default();
        let id = serde_json::from_str::<Value>(&first).unwrap_or_default()["id"].clone();
        let _ = server_out.write_all(format!("{ask}\n").as_bytes()).await;
        let reply = json!({"jsonrpc": "2.0", "id": id, "result": {}});
        let _ = server_out.write_all(format!("{reply}\n").as_bytes()).await;
        // The next lines are the client's refusal and its `initialized`.
        let mut got = Vec::new();
        for _ in 0..2 {
            got.push(lines.next_line().await.ok().flatten().unwrap_or_default());
        }
        let _ = tx.send(got);
    });
    let _conn = Connection::open(client_in, client_out, None)
        .await
        .expect("open");
    let got = rx.await.expect("lines");
    let refusal = got
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|m| m["id"] == 99)
        .expect("a reply to the server's request");
    assert_eq!(refusal["error"]["code"], -32601);
}
