//! Turning a `tools/call` result into the value a script sees.

use serde_json::{json, Value};

use crate::mcp_code::sandbox::{Attachment, ToolOutput};

/// Attachments per call and in total are bounded; the rest are noted but dropped.
const MAX_ATTACHMENTS: usize = 4;

/// Structured content wins, then text (parsed when it is JSON), then a list of blocks.
/// An `isError` result becomes `Err` with the tool's own text.
pub(super) fn output_of(result: &Value) -> Result<ToolOutput, String> {
    let blocks = result
        .get("content")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        let text = texts(blocks);
        return Err(if text.is_empty() {
            "the tool reported an error".into()
        } else {
            text
        });
    }
    let mut attachments = Vec::new();
    let described: Vec<Value> = blocks
        .iter()
        .map(|block| describe(block, &mut attachments))
        .collect();
    if let Some(structured) = result.get("structuredContent").filter(|v| !v.is_null()) {
        return Ok(ToolOutput {
            value: structured.clone(),
            attachments,
        });
    }
    let all_text = blocks
        .iter()
        .all(|block| block.get("type").and_then(Value::as_str) == Some("text"));
    let value = if all_text {
        text_value(&texts(blocks))
    } else {
        Value::Array(described)
    };
    Ok(ToolOutput { value, attachments })
}

fn texts(blocks: &[Value]) -> String {
    let parts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();
    parts.join("\n")
}

/// JSON text becomes a value so scripts can read fields; anything else stays a string.
fn text_value(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

fn describe(block: &Value, attachments: &mut Vec<Attachment>) -> Value {
    let kind = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match kind {
        "text" => json!({ "type": "text", "text": block.get("text").cloned().unwrap_or_default() }),
        "image" | "audio" => {
            let mime = block
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("application/octet-stream");
            let data = block
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let kept = attachments.len() < MAX_ATTACHMENTS;
            if kept {
                attachments.push(Attachment {
                    mime: mime.to_string(),
                    data: data.to_string(),
                });
            }
            json!({ "type": kind, "mimeType": mime, "attached": kept })
        }
        _ => json!({
            "type": kind,
            "uri": block.pointer("/resource/uri").cloned().unwrap_or_default(),
            "text": block.pointer("/resource/text").cloned().unwrap_or_default(),
        }),
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
