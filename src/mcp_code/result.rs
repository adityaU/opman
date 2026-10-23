//! Shaping a finished run into an MCP `tools/call` result.
//!
//! Two audiences read it. The model gets the script's value and its console output. The
//! web UI also wants every inner call with its input and output, which the model did not
//! ask for and should not pay tokens for: that goes in `structuredContent`, and again as
//! a delimited block marked for the user only, because the text is the one channel every
//! runner passes through to the card.

use serde_json::{json, Value};

use super::sandbox::{truncate, CallRecord, Capped, Finish, Report};

/// Tags around the call log in the user-only text block. The UI looks for these.
pub const CALLS_OPEN: &str = "<opman-code-calls>";
pub const CALLS_CLOSE: &str = "</opman-code-calls>";

pub fn to_mcp(report: &Report, max_result: usize) -> Value {
    let (main, failed) = main_text(report, max_result);
    let mut model_text = main;
    if !report.logs.is_empty() {
        if !model_text.is_empty() {
            model_text.push_str("\n\n");
        }
        model_text.push_str("Console:\n");
        for line in &report.logs {
            model_text.push_str(&format!("[{}] {}\n", line.level, line.text));
        }
    }
    let log = call_log(report);
    let mut content = vec![json!({ "type": "text", "text": model_text.trim_end() })];
    if !report.calls.is_empty() {
        content.push(json!({
            "type": "text",
            "text": format!("{CALLS_OPEN}{log}{CALLS_CLOSE}"),
            "annotations": { "audience": ["user"] },
        }));
    }
    for file in &report.attachments {
        content.push(json!({ "type": "image", "data": file.data, "mimeType": file.mime }));
    }
    json!({
        "content": content,
        "structuredContent": log,
        "isError": failed,
    })
}

fn main_text(report: &Report, max_result: usize) -> (String, bool) {
    match &report.finish {
        Finish::Value(text) if text.is_empty() => {
            if !report.calls.is_empty() {
                let fallback = summarize_calls(&report.calls);
                return truncate_with_note(&fallback, max_result);
            }
            if !report.logs.is_empty() {
                return (String::new(), false);
            }
            ("(the script returned nothing)".into(), false)
        }
        Finish::Value(text) => truncate_with_note(text, max_result),
        Finish::Failed(failure) => (failure.to_string(), true),
        Finish::TimedOut => ("Execution timed out.".into(), true),
        Finish::Cancelled => ("Execution cancelled.".into(), true),
    }
}

fn truncate_with_note(text: &str, max_result: usize) -> (String, bool) {
    let (kept, cut) = truncate(text, max_result);
    if !cut {
        return (kept.to_string(), false);
    }
    let note = format!(
        "\n\n[output truncated: showing {} of {} bytes. Return less, or filter in code.]",
        kept.len(),
        text.len()
    );
    (format!("{kept}{note}"), false)
}

/// When a script executes tool calls without an explicit `return`, surface the last completed
/// call's output (or a compact summary) so the model receives the result instead of nothing.
fn summarize_calls(calls: &[CallRecord]) -> String {
    if let [single] = calls {
        if !single.output.text.is_empty() {
            return single.output.text.clone();
        }
    }
    let mut lines = Vec::new();
    for call in calls {
        let status = if !call.finished {
            "cancelled"
        } else if call.ok {
            "ok"
        } else {
            "error"
        };
        if call.output.text.is_empty() {
            lines.push(format!("[{} ({status})]", call.tool));
        } else {
            lines.push(format!("[{} ({status})]\n{}", call.tool, call.output.text));
        }
    }
    lines.join("\n\n")
}

/// The call log, as it appears in `structuredContent` and in the user-only block.
pub fn call_log(report: &Report) -> Value {
    json!({
        "version": 1,
        "durationMs": u64::try_from(report.elapsed.as_millis()).unwrap_or(u64::MAX),
        "calls": report.calls.iter().map(call_json).collect::<Vec<_>>(),
    })
}

fn call_json(call: &CallRecord) -> Value {
    json!({
        "tool": call.tool,
        "input": capped_json(&call.input),
        "output": capped_json(&call.output),
        "ok": call.ok,
        "durationMs": call.duration_ms,
        "finished": call.finished,
    })
}

fn capped_json(text: &Capped) -> Value {
    json!({ "text": text.text, "truncated": text.truncated, "bytes": text.bytes })
}

#[cfg(test)]
#[path = "result_tests.rs"]
mod result_tests;
