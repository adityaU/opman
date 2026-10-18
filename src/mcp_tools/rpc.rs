//! JSON-RPC framing for the stdio transport: parse one line, build replies.

use serde::Deserialize;
use serde_json::{json, Value};

/// One incoming line. A message without an `id` is a notification and gets no reply.
#[derive(Debug, Deserialize)]
pub struct Incoming {
    pub method: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub id: Option<Value>,
}

pub fn parse_error(detail: &str) -> Value {
    error_reply(&Value::Null, -32700, &format!("Parse error: {detail}"))
}

pub fn error_reply(id: &Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub fn result_reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn progress(token: &Value, step: u64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": { "progressToken": token, "progress": step, "message": message },
    })
}

/// The `initialize` result. The client's protocol version is echoed back, because
/// structured tool output needs 2025-06-18 or newer and older clients ignore the extra.
pub fn initialize_result(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("2025-06-18");
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "opman-tools", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Common coding tools: run shell commands, read, write and edit files, \
                         list folders, find files, search text, and fetch web pages. Relative \
                         paths start at the project folder.",
    })
}
