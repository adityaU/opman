//! What a tool returns: text for the model, structured data for code, and an error flag.

use serde::Serialize;
use serde_json::{json, Value};
use std::io;

/// A failure the model can read and act on. It becomes an `isError` result, not a
/// JSON-RPC error, so the model sees the message.
#[derive(Debug, PartialEq, Eq)]
pub struct ToolError(String);

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// An I/O failure with the path it happened on, in words a model understands.
    pub fn io(action: &str, shown_path: &str, error: &io::Error) -> Self {
        let reason = match error.kind() {
            io::ErrorKind::NotFound => "no such file or directory".to_string(),
            io::ErrorKind::PermissionDenied => "permission denied".to_string(),
            _ => error.to_string(),
        };
        Self(format!("cannot {action} {shown_path}: {reason}"))
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct ToolOutput {
    pub text: String,
    pub data: Value,
    pub is_error: bool,
}

pub type ToolResult = Result<ToolOutput, ToolError>;

impl ToolOutput {
    pub fn ok<T: Serialize>(text: String, data: &T) -> ToolResult {
        Self::build(text, data, false)
    }

    /// A result that carries data but still counts as a failure, such as a timeout.
    pub fn failed<T: Serialize>(text: String, data: &T) -> ToolResult {
        Self::build(text, data, true)
    }

    fn build<T: Serialize>(text: String, data: &T, is_error: bool) -> ToolResult {
        let data = serde_json::to_value(data)
            .map_err(|e| ToolError::new(format!("cannot encode result: {e}")))?;
        Ok(Self {
            text,
            data,
            is_error,
        })
    }
}

/// The MCP `tools/call` result. Errors carry no `structuredContent`: it would not match
/// the output schema.
pub fn call_result(result: ToolResult) -> Value {
    match result {
        Ok(out) if out.is_error => {
            json!({ "content": [{ "type": "text", "text": out.text }], "isError": true })
        }
        Ok(out) => json!({
            "content": [{ "type": "text", "text": out.text }],
            "structuredContent": out.data,
            "isError": false,
        }),
        Err(error) => {
            json!({ "content": [{ "type": "text", "text": error.0 }], "isError": true })
        }
    }
}
