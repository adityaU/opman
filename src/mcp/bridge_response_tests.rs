//! Wave-3 breadth coverage for the terminal MCP bridge (`bridge.rs`): the
//! JSON-RPC response builders across id-type variants, `tool_call_response`
//! Ok/Err content shapes, `write_jsonrpc_stdout`, and the read-loop driving
//! multiple concurrent `tools/call`s against a dead socket.
use super::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn buf() -> Arc<tokio::sync::Mutex<Vec<u8>>> {
    Arc::new(tokio::sync::Mutex::new(Vec::<u8>::new()))
}

fn dead_sock() -> Arc<PathBuf> {
    Arc::new(PathBuf::from("/nonexistent/opman-bridge-response-test.sock"))
}

// ── response builders across id variants ─────────────────────────────────────

#[test]
fn initialize_response_preserves_string_id() {
    let v = initialize_response(&json!("abc"));
    assert_eq!(v["id"], json!("abc"));
    assert_eq!(v["result"]["serverInfo"]["name"], "opman-terminal");
    assert_eq!(v["result"]["protocolVersion"], "2024-11-05");
    assert!(v["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn initialize_response_preserves_null_id() {
    let v = initialize_response(&json!(null));
    assert_eq!(v["id"], json!(null));
}

#[test]
fn tools_list_response_lists_terminal_tools() {
    let v = tools_list_response(&json!(42));
    assert_eq!(v["id"], json!(42));
    let tools = v["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);
    // Every tool has a name + inputSchema.
    for t in tools {
        assert!(t["name"].is_string());
        assert!(t["inputSchema"].is_object());
    }
}

#[test]
fn method_not_found_interpolates_method_and_id() {
    let v = method_not_found_response("do/thing", &json!("x-1"));
    assert_eq!(v["error"]["code"], -32601);
    assert!(v["error"]["message"].as_str().unwrap().contains("do/thing"));
    assert_eq!(v["id"], json!("x-1"));
}

#[test]
fn parse_error_always_null_id() {
    let v = parse_error_response("unexpected token");
    assert_eq!(v["error"]["code"], -32700);
    assert!(v["error"]["message"].as_str().unwrap().contains("unexpected token"));
    assert_eq!(v["id"], json!(null));
}

// ── tool_call_response content shapes ────────────────────────────────────────

#[test]
fn tool_call_response_ok_with_object_content() {
    let content = json!({ "structured": [1, 2, 3] });
    let v = tool_call_response(Ok(content.clone()), &json!(1));
    assert_eq!(v["result"]["content"], content);
    assert!(v["result"].get("isError").is_none());
}

#[test]
fn tool_call_response_ok_with_array_content() {
    let content = json!([{ "type": "text", "text": "hello" }]);
    let v = tool_call_response(Ok(content.clone()), &json!(2));
    assert_eq!(v["result"]["content"], content);
}

#[test]
fn tool_call_response_err_sets_is_error_and_prefix() {
    let v = tool_call_response(Err(anyhow::anyhow!("socket gone")), &json!(3));
    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("Error: "));
    assert!(text.contains("socket gone"));
}

// ── write_jsonrpc_stdout ─────────────────────────────────────────────────────

#[tokio::test]
async fn write_jsonrpc_stdout_appends_newline() {
    let out = buf();
    write_jsonrpc_stdout(&out, &json!({ "arr": [1, 2] })).await;
    let s = String::from_utf8(out.lock().await.clone()).unwrap();
    assert!(s.contains("\"arr\":[1,2]"));
    assert!(s.ends_with('\n'));
}

// ── request deserialization ──────────────────────────────────────────────────

#[test]
fn request_with_object_id_deserializes() {
    let r: McpJsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"tools/call","id":{"n":1}}"#).unwrap();
    assert_eq!(r.method, "tools/call");
    assert_eq!(r.id, json!({ "n": 1 }));
    assert!(r.params.is_none());
}

// ── read-loop: multiple concurrent tools/call against a dead socket ───────────

#[tokio::test]
async fn run_bridge_over_two_tool_calls_both_get_ids_back() {
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"terminal_read\",\"arguments\":{}},\"id\":101}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"terminal_read\",\"arguments\":{}},\"id\":102}\n",
    );
    let out = buf();
    run_bridge_over(
        input.as_bytes(),
        Arc::clone(&out),
        dead_sock(),
        Arc::new(Some("sess-x".to_string())),
    )
    .await
    .unwrap();

    // Spawned tasks may finish after the loop returns; poll briefly.
    let mut s = String::new();
    for _ in 0..5000 {
        s = String::from_utf8(out.lock().await.clone()).unwrap();
        if s.matches("isError").count() >= 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(s.contains("\"id\":101"), "missing id 101: {s}");
    assert!(s.contains("\"id\":102"), "missing id 102: {s}");
}

#[tokio::test]
async fn run_bridge_over_initialize_then_eof() {
    let input = "{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"id\":\"init\"}\n";
    let out = buf();
    run_bridge_over(input.as_bytes(), Arc::clone(&out), dead_sock(), Arc::new(None))
        .await
        .unwrap();
    let s = String::from_utf8(out.lock().await.clone()).unwrap();
    assert!(s.contains("opman-terminal"));
    assert!(s.contains("\"id\":\"init\""));
}
