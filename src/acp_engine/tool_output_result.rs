//! Text extraction from MCP results carried inside ACP `rawOutput`.

use serde_json::{Map, Value};

const RAW_OUTPUT_LOCK: &str = "_acpRawOutputIsAuthoritative";

/// Read a `CallToolResult` as user-visible text, preferring its text blocks and then its
/// structured result. The latter is serialized without its surrounding MCP envelope.
pub(super) fn mcp_text(raw: &Value) -> Option<String> {
    if let Some(error) = raw.get("error").filter(|error| !error.is_null()) {
        return Some(match error.get("message").and_then(Value::as_str) {
            Some(message) => message.to_string(),
            None => error.to_string(),
        });
    }

    let result = match raw.get("result").filter(|result| !result.is_null()) {
        Some(result) => result,
        None => raw,
    };
    let blocks = result.get("content").and_then(Value::as_array).or_else(|| {
        raw.as_array().filter(|items| {
            !items.is_empty()
                && items
                    .iter()
                    .all(|item| item.get("type").and_then(Value::as_str).is_some())
        })
    });
    if let Some(blocks) = blocks {
        let text = blocks
            .iter()
            .filter_map(|block| block.get("text")?.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Some(text);
        }
    }

    if let Some(structured) = result
        .get("structuredContent")
        .or_else(|| raw.get("structuredContent"))
    {
        if is_call_log_value(structured) {
            return None;
        }
        return serde_json::to_string(structured).ok();
    }
    blocks.map(|_| String::new())
}

/// Select an output channel without retaining a second copy of output text in metadata.
pub(super) fn merge_output(state: &mut Map<String, Value>, update: &Value) {
    let authoritative = update
        .get("rawOutput")
        .filter(|raw| !raw.is_null())
        .filter(|_| raw_output_is_native(update))
        .and_then(|_| super::raw_output(update));
    if let Some(output) = authoritative {
        state.insert("output".into(), Value::String(output));
        state.insert(RAW_OUTPUT_LOCK.into(), Value::Bool(true));
        return;
    }
    if state.get(RAW_OUTPUT_LOCK).and_then(Value::as_bool) == Some(true) {
        return;
    }
    if let Some(output) = super::raw_output(update) {
        state.insert("output".into(), Value::String(output));
    }
}

pub(super) fn is_call_log(raw: &Value) -> bool {
    let result = raw.get("result").unwrap_or(raw);
    result
        .get("structuredContent")
        .is_some_and(is_call_log_value)
}

fn is_call_log_value(value: &Value) -> bool {
    value.get("version").and_then(Value::as_u64).is_some()
        && value.get("calls").and_then(Value::as_array).is_some()
}

fn raw_output_is_native(update: &Value) -> bool {
    let Some(raw) = update.get("rawOutput").filter(|raw| !raw.is_null()) else {
        return false;
    };
    if raw.is_string() {
        return true;
    }
    if super::exit_status(raw).is_some() {
        return false;
    }
    if let Some(output) = super::command_output(raw) {
        return !output.is_empty();
    }
    if mcp_text(raw).is_some() {
        return true;
    }
    super::content_text(update).is_none() && serde_json::to_string(raw).is_ok()
}
