//! Generated tests for JSON-RPC 2.0 protocol types.

use super::*;

#[test]
fn deserialize_full_request() {
    let req: JsonRpcRequest = serde_json::from_str(
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"x"}}"#,
    )
    .unwrap();
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, Some(serde_json::json!(7)));
    assert_eq!(req.method, "tools/call");
    assert_eq!(req.params.unwrap()["name"], "x");
}

#[test]
fn deserialize_request_without_id_or_params() {
    // id and params default to None (notification style).
    let req: JsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"initialized"}"#).unwrap();
    assert!(req.id.is_none());
    assert!(req.params.is_none());
    assert_eq!(req.method, "initialized");
}

#[test]
fn deserialize_request_string_id() {
    let req: JsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":"abc","method":"ping"}"#).unwrap();
    assert_eq!(req.id, Some(serde_json::json!("abc")));
}

#[test]
fn deserialize_request_missing_method_fails() {
    assert!(serde_json::from_str::<JsonRpcRequest>(r#"{"jsonrpc":"2.0","id":1}"#).is_err());
}

#[test]
fn success_response_serializes_result_only() {
    let resp =
        JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"ok": true}));
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 1);
    assert_eq!(v["result"]["ok"], true);
    // error is None -> skipped.
    assert!(v.get("error").is_none());
}

#[test]
fn error_response_serializes_error_only() {
    let resp = JsonRpcResponse::error(Some(serde_json::json!("id-2")), METHOD_NOT_FOUND, "nope");
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["id"], "id-2");
    assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
    assert_eq!(v["error"]["message"], "nope");
    // result is None -> skipped; error.data is None -> skipped.
    assert!(v.get("result").is_none());
    assert!(v["error"].get("data").is_none());
}

#[test]
fn response_with_null_id_omits_id_field() {
    let resp = JsonRpcResponse::error(None, PARSE_ERROR, "bad json");
    let v = serde_json::to_value(&resp).unwrap();
    // id is None -> skipped entirely.
    assert!(v.get("id").is_none());
    assert_eq!(v["error"]["code"], PARSE_ERROR);
}

#[test]
fn error_code_constants_have_expected_values() {
    assert_eq!(PARSE_ERROR, -32700);
    assert_eq!(INVALID_REQUEST, -32600);
    assert_eq!(METHOD_NOT_FOUND, -32601);
    assert_eq!(INTERNAL_ERROR, -32603);
    assert_eq!(MCP_PROTOCOL_VERSION, "2024-11-05");
    assert_eq!(SERVER_NAME, "opman-web-mcp");
    assert!(!SERVER_VERSION.is_empty());
}
