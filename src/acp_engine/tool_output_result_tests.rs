//! Regression coverage for MCP result bodies inside ACP tool updates.

use serde_json::{json, Value};

use super::super::tool::merge;

fn code_tool() -> Value {
    json!({
        "type": "tool",
        "tool": "mcp__opman-code__execute",
        "state": { "status": "running", "input": {} }
    })
}

#[test]
fn text_content_wins_and_structured_content_is_kept_as_metadata() {
    let structured = json!({ "result": "rendered" });
    let mut part = code_tool();
    merge(
        &mut part,
        &json!({
            "status": "completed",
            "rawOutput": {
                "result": {
                    "content": [{ "type": "text", "text": "UI block rendered successfully" }],
                    "structuredContent": structured
                }
            }
        }),
    );

    assert_eq!(part["state"]["output"], "UI block rendered successfully");
    assert_eq!(part["state"]["metadata"]["structuredContent"], structured);
}

#[test]
fn ordinary_structured_content_without_text_is_emitted_without_the_mcp_envelope() {
    let structured = json!({ "result": "UI block rendered successfully" });
    for raw in [
        json!({ "structuredContent": structured }),
        json!({ "result": { "structuredContent": structured } }),
    ] {
        let mut part = code_tool();
        merge(
            &mut part,
            &json!({ "status": "completed", "rawOutput": raw }),
        );
        assert_eq!(
            part["state"]["output"],
            "{\"result\":\"UI block rendered successfully\"}"
        );
        assert_eq!(part["state"]["metadata"]["structuredContent"], structured);
    }
}

#[test]
fn structured_call_log_is_kept_as_metadata_not_shown_as_the_return_value() {
    let calls = json!({ "version": 1, "calls": [{ "name": "ui_render" }] });
    let mut part = code_tool();
    merge(
        &mut part,
        &json!({
            "status": "completed",
            "rawOutput": { "result": { "structuredContent": calls } }
        }),
    );

    assert!(part["state"].get("output").is_none());
    assert_eq!(part["state"]["metadata"]["structuredContent"], calls);
}

#[test]
fn call_titles_and_meta_prompts_are_not_final_tool_output() {
    let mut part = code_tool();
    merge(
        &mut part,
        &json!({
            "title": "Render UI confirmation card using ui_render",
            "_meta": { "prompt": "Render UI confirmation card using ui_render" },
            "status": "completed"
        }),
    );

    assert!(part["state"].get("output").is_none());
    assert_eq!(
        part["state"]["title"],
        "Render UI confirmation card using ui_render"
    );
}

#[test]
fn content_only_delta_does_not_replace_completed_raw_output() {
    let mut part = code_tool();
    merge(
        &mut part,
        &json!({ "status": "completed", "rawOutput": "native result" }),
    );
    merge(
        &mut part,
        &json!({
            "title": "Render UI confirmation card using ui_render",
            "content": [{
                "type": "content",
                "content": { "type": "text", "text": "Render UI confirmation card using ui_render" }
            }]
        }),
    );
    assert_eq!(part["state"]["output"], "native result");
    assert_eq!(part["state"]["_acpRawOutputIsAuthoritative"], true);
}

#[test]
fn a_later_raw_output_update_replaces_prior_native_output() {
    let mut part = code_tool();
    merge(&mut part, &json!({ "rawOutput": "intermediate" }));
    merge(&mut part, &json!({ "rawOutput": "final" }));
    assert_eq!(part["state"]["output"], "final");
}
