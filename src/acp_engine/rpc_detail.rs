//! The reason inside a JSON-RPC error object, kept whole.

use serde_json::Value;

/// What the error's `data` says, as text: `data.details` when there is one, a bare string
/// as it is, and any other value as compact JSON. `None` when there is no data to show.
pub(super) fn detail(error: &Value) -> Option<String> {
    let data = error.get("data").filter(|data| !data.is_null())?;
    if let Some(details) = data.get("details").and_then(Value::as_str) {
        return Some(details.to_string());
    }
    match data {
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
#[path = "rpc_detail_tests.rs"]
mod rpc_detail_tests;
