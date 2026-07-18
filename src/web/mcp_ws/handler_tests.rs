//! Generated tests for MCP method dispatch.
//!
//! `websocket_handler` and `handle_mcp_session` perform a WebSocket upgrade and
//! run an infinite read loop; they are not exercised here (see module report).
//! `dispatch_method` and the per-method handlers are pure async functions and
//! are driven directly with a test `ServerState`.

use super::*;
use crate::web::test_support::test_server_state;

fn req(v: serde_json::Value) -> JsonRpcRequest {
    serde_json::from_value(v).unwrap()
}

fn to_value(resp: &JsonRpcResponse) -> serde_json::Value {
    serde_json::to_value(resp).unwrap()
}

#[tokio::test]
async fn dispatch_initialize() {
    let state = test_server_state();
    let r = dispatch_method(&state, &req(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))).await;
    let v = to_value(&r);
    assert_eq!(v["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
    assert_eq!(v["result"]["serverInfo"]["name"], SERVER_NAME);
}

#[tokio::test]
async fn dispatch_initialized_notification() {
    let state = test_server_state();
    let r = dispatch_method(&state, &req(serde_json::json!({"jsonrpc":"2.0","id":2,"method":"initialized"}))).await;
    let v = to_value(&r);
    assert_eq!(v["result"], serde_json::json!({}));
}

#[tokio::test]
async fn dispatch_tools_list() {
    let state = test_server_state();
    let r = dispatch_method(&state, &req(serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))).await;
    let v = to_value(&r);
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 8);
}

#[tokio::test]
async fn dispatch_ping() {
    let state = test_server_state();
    let r = dispatch_method(&state, &req(serde_json::json!({"jsonrpc":"2.0","id":4,"method":"ping"}))).await;
    assert_eq!(to_value(&r)["result"], serde_json::json!({}));
}

#[tokio::test]
async fn dispatch_unknown_method_is_method_not_found() {
    let state = test_server_state();
    let r = dispatch_method(&state, &req(serde_json::json!({"jsonrpc":"2.0","id":5,"method":"bogus"}))).await;
    let v = to_value(&r);
    assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
    assert!(v["error"]["message"].as_str().unwrap().contains("bogus"));
}

#[tokio::test]
async fn dispatch_tools_call_missing_name() {
    let state = test_server_state();
    let r = dispatch_method(
        &state,
        &req(serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{}})),
    )
    .await;
    let v = to_value(&r);
    assert_eq!(v["error"]["code"], INVALID_REQUEST);
}

#[tokio::test]
async fn dispatch_tools_call_terminal_list_ok() {
    let state = test_server_state();
    let r = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":7,"method":"tools/call",
            "params":{"name":"web_terminal_list","arguments":{}}
        })),
    )
    .await;
    let v = to_value(&r);
    // Result content[0].text is a JSON string with terminals/count.
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    let inner: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(inner["count"], 0);
    assert!(v["result"].get("isError").is_none());
}

#[tokio::test]
async fn dispatch_tools_call_unknown_tool_is_error_content() {
    let state = test_server_state();
    let r = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":8,"method":"tools/call",
            "params":{"name":"no_such_tool","arguments":{}}
        })),
    )
    .await;
    let v = to_value(&r);
    assert_eq!(v["result"]["isError"], true);
    assert!(v["result"]["content"][0]["text"].as_str().unwrap().contains("Unknown tool"));
}

#[tokio::test]
async fn dispatch_tools_call_terminal_read_missing_id_is_error_content() {
    let state = test_server_state();
    let r = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":9,"method":"tools/call",
            "params":{"name":"web_terminal_read","arguments":{}}
        })),
    )
    .await;
    let v = to_value(&r);
    assert_eq!(v["result"]["isError"], true);
}

#[tokio::test]
async fn dispatch_tools_call_defaults_arguments_when_absent() {
    // No "arguments" key -> defaults to {} and terminal_list still works.
    let state = test_server_state();
    let r = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":10,"method":"tools/call",
            "params":{"name":"web_terminal_list"}
        })),
    )
    .await;
    assert!(to_value(&r)["result"]["content"][0]["text"].as_str().is_some());
}
