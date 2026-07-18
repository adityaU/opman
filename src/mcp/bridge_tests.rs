use super::*;
use serde_json::json;

#[test]
fn deserializes_valid_request_with_params() {
    let req: McpJsonRpcRequest = serde_json::from_str(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"x"},"id":7}"#,
    )
    .unwrap();
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.method, "tools/call");
    assert_eq!(req.id, json!(7));
    assert!(req.params.is_some());
}

#[test]
fn deserializes_request_without_params_defaults_none() {
    let req: McpJsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"initialize","id":"abc"}"#).unwrap();
    assert_eq!(req.method, "initialize");
    assert!(req.params.is_none());
    assert_eq!(req.id, json!("abc"));
}

#[test]
fn deserialize_rejects_bad_json() {
    let r: Result<McpJsonRpcRequest, _> = serde_json::from_str("{not json");
    assert!(r.is_err());
}

#[test]
fn initialize_response_shape() {
    let v = initialize_response(&json!(1));
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], json!(1));
    assert_eq!(v["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(v["result"]["serverInfo"]["name"], "opman-terminal");
    assert_eq!(v["result"]["serverInfo"]["version"], "1.0.0");
    assert!(v["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn tools_list_response_has_tools_array() {
    let v = tools_list_response(&json!("id-1"));
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], json!("id-1"));
    let tools = v["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);
}

#[test]
fn method_not_found_response_shape() {
    let v = method_not_found_response("frobnicate", &json!(9));
    assert_eq!(v["error"]["code"], -32601);
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("frobnicate"));
    assert_eq!(v["id"], json!(9));
}

#[test]
fn parse_error_response_shape() {
    let v = parse_error_response("boom");
    assert_eq!(v["error"]["code"], -32700);
    assert!(v["error"]["message"].as_str().unwrap().contains("boom"));
    assert_eq!(v["id"], json!(null));
}

#[test]
fn tool_call_response_ok() {
    let content = json!([{ "type": "text", "text": "hi" }]);
    let v = tool_call_response(Ok(content.clone()), &json!(3));
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], json!(3));
    assert_eq!(v["result"]["content"], content);
    assert!(v["result"].get("isError").is_none());
}

#[test]
fn tool_call_response_err() {
    let v = tool_call_response(Err(anyhow::anyhow!("kaboom")), &json!(4));
    assert_eq!(v["result"]["isError"], true);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("kaboom"));
    assert_eq!(v["result"]["content"][0]["type"], "text");
}

#[tokio::test]
async fn write_jsonrpc_stdout_writes_without_panicking() {
    let out = std::sync::Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    // Exercises the happy path: serialize + write + newline + flush.
    write_jsonrpc_stdout(&out, &json!({"ok": true})).await;
}
