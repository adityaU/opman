//! The `execute` tool: its definition and one call of it.

use std::sync::Arc;

use serde_json::{json, Value};

use super::catalog::Catalog;
use super::client::{Pool, PoolHost};
use super::describe::{self, DEFAULT_BUDGET};
use super::result;
use super::sandbox::{self, Cancel, Limits};

pub const TOOL_NAME: &str = "execute";

/// The definition `tools/list` returns, with the description built from `catalog`.
pub fn definition(catalog: &Catalog) -> Value {
    let (description, _) = describe::build(catalog, DEFAULT_BUDGET);
    json!({
        "name": TOOL_NAME,
        "title": "Execute JavaScript",
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "The body of an async function. Use await, return the result."
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 100,
                    "description": "Wall-clock limit in milliseconds. Default 60000, at most 600000."
                }
            },
            "required": ["code"]
        },
    })
}

/// The validated arguments of one call.
#[derive(Debug, PartialEq, Eq)]
pub struct Input<'a> {
    pub code: &'a str,
    pub timeout_ms: Option<u64>,
}

/// Read `arguments`. The error text goes back to the model as the tool's own error.
pub fn parse(arguments: Option<&Value>) -> Result<Input<'_>, String> {
    let code = arguments
        .and_then(|args| args.get("code"))
        .and_then(Value::as_str)
        .ok_or("`code` is required and must be a string.")?;
    let timeout_ms = match arguments.and_then(|args| args.get("timeout_ms")) {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or("`timeout_ms` must be a positive whole number.")?,
        ),
    };
    Ok(Input { code, timeout_ms })
}

/// Run one script against the pool's current catalog and shape the MCP result.
pub async fn call(pool: &Arc<Pool>, input: Input<'_>, cancel: Cancel) -> Value {
    let catalog = Arc::new(pool.catalog().await);
    let host = Arc::new(PoolHost::new(pool.clone(), catalog));
    let limits = Limits::default().with_timeout_ms(input.timeout_ms);
    let report = sandbox::run(input.code, host, limits, cancel).await;
    result::to_mcp(&report, limits.max_result)
}

pub fn failure(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

#[cfg(test)]
#[path = "execute_tests.rs"]
mod execute_tests;
