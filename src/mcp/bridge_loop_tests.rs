//! Wave-2 coverage for the MCP bridge read-loop (`run_bridge_over`).
use super::*;
use std::path::PathBuf;
use std::sync::Arc;

fn buf() -> Arc<tokio::sync::Mutex<Vec<u8>>> {
    Arc::new(tokio::sync::Mutex::new(Vec::<u8>::new()))
}

fn dead_sock() -> Arc<PathBuf> {
    Arc::new(std::path::PathBuf::from(
        "/nonexistent/opman-bridge-test.sock",
    ))
}

#[tokio::test]
async fn run_bridge_over_non_tool_methods() {
    // Covers: blank line skip, parse error, initialize, notification (no reply),
    // tools/list, unknown method, then EOF.
    let input = concat!(
        "\n",
        "garbage-not-json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"id\":1}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"id\":0}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\",\"id\":2}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"bogus\",\"id\":3}\n",
    );
    let out = buf();
    run_bridge_over(
        input.as_bytes(),
        Arc::clone(&out),
        dead_sock(),
        Arc::new(None),
    )
    .await
    .unwrap();
    let s = String::from_utf8(out.lock().await.clone()).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    // parse error + initialize + tools/list + unknown = 4 synchronous responses.
    assert_eq!(lines.len(), 4, "got: {s}");
    assert!(s.contains("Parse error"));
    assert!(s.contains("opman-terminal"));
    assert!(s.contains("Method not found: bogus"));
}

#[tokio::test]
async fn run_bridge_over_tools_call_spawns_and_writes() {
    // A tools/call against a dead socket: the spawned task runs, handle_tool_call
    // errors (connection refused), and an isError response is written back.
    let input = "{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"terminal_read\",\"arguments\":{}},\"id\":9}\n";
    let out = buf();
    run_bridge_over(
        input.as_bytes(),
        Arc::clone(&out),
        dead_sock(),
        Arc::new(Some("sess-1".to_string())),
    )
    .await
    .unwrap();
    // The spawned task may finish after the loop returns; poll briefly.
    let mut s = String::new();
    for _ in 0..2000 {
        s = String::from_utf8(out.lock().await.clone()).unwrap();
        if !s.is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        !s.is_empty(),
        "expected a response from the spawned tool call"
    );
    assert!(
        s.contains("\"id\":9"),
        "response should carry the request id: {s}"
    );
}

#[tokio::test]
async fn run_bridge_over_eof_immediately() {
    let out = buf();
    run_bridge_over(&b""[..], Arc::clone(&out), dead_sock(), Arc::new(None))
        .await
        .unwrap();
    assert!(out.lock().await.is_empty());
}

#[tokio::test]
async fn write_jsonrpc_stdout_generic_vec() {
    let out = buf();
    write_jsonrpc_stdout(&out, &serde_json::json!({"a":1})).await;
    let s = String::from_utf8(out.lock().await.clone()).unwrap();
    assert!(s.contains("\"a\":1"));
    assert!(s.ends_with('\n'));
}
