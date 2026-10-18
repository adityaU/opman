use std::time::Duration;

use serde_json::Value;

use super::*;
use crate::mcp_code::sandbox::{Attachment, CallRecord, Capped, Finish, Report};

fn report(finish: Finish, calls: Vec<CallRecord>) -> Report {
    Report {
        finish,
        logs: Vec::new(),
        calls,
        attachments: Vec::new(),
        elapsed: Duration::from_millis(12),
    }
}

fn call(tool: &str, input: &str, output: &str, ok: bool) -> CallRecord {
    CallRecord {
        tool: tool.into(),
        input: Capped::new(input, 8192),
        output: Capped::new(output, 8192),
        ok,
        duration_ms: 5,
        finished: true,
    }
}

fn first_text(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap_or_default()
}

#[test]
fn a_plain_value_is_one_text_block() {
    let result = to_mcp(&report(Finish::Value("hi".into()), vec![]), 1000);
    assert_eq!(first_text(&result), "hi");
    assert_eq!(result["isError"], false);
    assert_eq!(result["content"].as_array().map(Vec::len), Some(1));
}

#[test]
fn empty_value_says_so() {
    let result = to_mcp(&report(Finish::Value(String::new()), vec![]), 1000);
    assert_eq!(first_text(&result), "(the script returned nothing)");
}

#[test]
fn long_results_are_truncated_with_a_marker() {
    let result = to_mcp(&report(Finish::Value("x".repeat(500)), vec![]), 100);
    let text = first_text(&result);
    assert!(text.starts_with(&"x".repeat(100)));
    assert!(text.contains("[output truncated: showing 100 of 500 bytes"));
}

#[test]
fn failures_set_is_error() {
    for (finish, text) in [
        (Finish::TimedOut, "Execution timed out."),
        (Finish::Cancelled, "Execution cancelled."),
    ] {
        let result = to_mcp(&report(finish, vec![]), 100);
        assert_eq!(first_text(&result), text);
        assert_eq!(result["isError"], true);
    }
}

#[test]
fn console_lines_follow_the_value() {
    let mut r = report(Finish::Value("v".into()), vec![]);
    r.logs.push(crate::mcp_code::sandbox::LogLine {
        level: "warn".into(),
        text: "careful".into(),
    });
    let text = first_text(&to_mcp(&r, 100)).to_string();
    assert_eq!(text, "v\n\nConsole:\n[warn] careful");
}

#[test]
fn call_metadata_has_tool_input_output_duration_ok() {
    let r = report(
        Finish::Value("v".into()),
        vec![
            call("time.now", "{}", "noon", true),
            call("m.fail", "{\"a\":1}", "boom", false),
        ],
    );
    let result = to_mcp(&r, 100);
    let calls = &result["structuredContent"]["calls"];
    assert_eq!(calls[0]["tool"], "time.now");
    assert_eq!(calls[0]["input"]["text"], "{}");
    assert_eq!(calls[0]["output"]["text"], "noon");
    assert_eq!(calls[0]["ok"], true);
    assert_eq!(calls[0]["durationMs"], 5);
    assert_eq!(calls[1]["ok"], false);
    assert_eq!(calls[1]["output"]["text"], "boom");
    assert_eq!(result["structuredContent"]["durationMs"], 12);
}

#[test]
fn the_user_block_repeats_the_log_and_is_marked_for_the_user() {
    let r = report(
        Finish::Value("v".into()),
        vec![call("a.b", "{}", "ok", true)],
    );
    let result = to_mcp(&r, 100);
    let block = &result["content"][1];
    assert_eq!(block["annotations"]["audience"][0], "user");
    let text = block["text"].as_str().unwrap_or_default();
    let body = text
        .strip_prefix(CALLS_OPEN)
        .and_then(|rest| rest.strip_suffix(CALLS_CLOSE))
        .expect("tags");
    let parsed: Value = serde_json::from_str(body).expect("json");
    assert_eq!(parsed, result["structuredContent"]);
}

#[test]
fn truncated_inner_output_is_flagged_in_the_metadata() {
    let big = "y".repeat(20_000);
    let mut record = call("m.big", "{}", "", true);
    record.output = Capped::new(&big, 8192);
    let result = to_mcp(&report(Finish::Value("v".into()), vec![record]), 100);
    let output = &result["structuredContent"]["calls"][0]["output"];
    assert_eq!(output["truncated"], true);
    assert_eq!(output["bytes"], 20_000);
    assert_eq!(output["text"].as_str().map(str::len), Some(8192));
}

#[test]
fn attachments_become_image_blocks() {
    let mut r = report(Finish::Value("v".into()), vec![]);
    r.attachments.push(Attachment {
        mime: "image/png".into(),
        data: "AAAA".into(),
    });
    let result = to_mcp(&r, 100);
    let block = &result["content"][1];
    assert_eq!(block["type"], "image");
    assert_eq!(block["mimeType"], "image/png");
}
