//! Wave-3 breadth coverage for the MCP UI render server: every documented block
//! type through `handle_ui_render`, every delta-operation permutation, dispatch
//! edge cases, request deserialization, and the tool-definition schema.
use super::*;
use serde_json::json;

/// The full documented block-type vocabulary (matches the schema description in
/// `tool_definitions`). Each one is validated + echoed by `handle_ui_render`.
const ALL_BLOCK_TYPES: &[&str] = &[
    "card",
    "table",
    "kv",
    "status",
    "progress",
    "alert",
    "button",
    "form",
    "markdown",
    "steps",
    "divider",
    "code",
    "metric",
    "grid",
    "flex",
    "image",
    "pdf",
    "link",
    "accordion",
    "chart",
    "tabs",
    "callout",
    "badge",
    "blockquote",
    "list",
    "stat-group",
    "diff",
    "timeline",
    "terminal",
    "file-tree",
    "avatar",
    "tag-group",
    "toggle",
    "video",
    "audio",
    "separator",
    "mermaid",
];

#[test]
fn every_block_type_renders_ok() {
    for ty in ALL_BLOCK_TYPES {
        let args = json!({
            "title": format!("T-{ty}"),
            "blocks": [{ "type": ty, "data": { "sample": true } }]
        });
        let out = handle_ui_render(&args);
        let text = out[0]["text"].as_str().unwrap();
        assert!(
            text.contains(&format!("Rendered UI: T-{ty} (1 blocks)")),
            "block type {ty} did not render: {text}"
        );
    }
}

#[test]
fn heterogeneous_multi_block_payload_counts_all() {
    let blocks: Vec<serde_json::Value> = ALL_BLOCK_TYPES
        .iter()
        .map(|ty| json!({ "type": ty, "data": {} }))
        .collect();
    let n = blocks.len();
    let out = handle_ui_render(&json!({ "title": "big", "blocks": blocks }));
    let text = out[0]["text"].as_str().unwrap();
    assert!(text.contains(&format!("({n} blocks)")), "got: {text}");
}

// ── delta-operation permutations ─────────────────────────────────────────────

#[test]
fn delta_replace_append_update_all_reported() {
    for op in ["replace", "append", "update"] {
        let args = json!({
            "title": "D",
            "blocks": [{ "type": "steps", "data": {} }],
            "render_id": "rid-9",
            "operation": op
        });
        let text = handle_ui_render(&args)[0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains(&format!("{op}:rid-9")), "got: {text}");
    }
}

#[test]
fn render_id_without_operation_uses_plain_desc() {
    // (Some(rid), None) → falls through to the `_` arm: no op suffix.
    let args = json!({
        "title": "P",
        "blocks": [{ "type": "card", "data": {} }],
        "render_id": "only-rid"
    });
    let text = handle_ui_render(&args)[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(text, "Rendered UI: P (1 blocks)");
    assert!(!text.contains("only-rid"));
}

#[test]
fn operation_without_render_id_uses_plain_desc() {
    // (None, Some(op)) → also the `_` arm.
    let args = json!({
        "title": "Q",
        "blocks": [{ "type": "card", "data": {} }],
        "operation": "append"
    });
    let text = handle_ui_render(&args)[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(text, "Rendered UI: Q (1 blocks)");
}

// ── validation error branches ────────────────────────────────────────────────

#[test]
fn second_block_missing_type_reports_its_index() {
    let args = json!({
        "title": "x",
        "blocks": [
            { "type": "card", "data": {} },
            { "data": {} }
        ]
    });
    let text = handle_ui_render(&args)[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("Block 1 missing 'type'"), "got: {text}");
}

#[test]
fn third_block_missing_data_reports_its_index() {
    let args = json!({
        "title": "x",
        "blocks": [
            { "type": "card", "data": {} },
            { "type": "kv", "data": {} },
            { "type": "alert" }
        ]
    });
    let text = handle_ui_render(&args)[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("Block 2 missing 'data'"), "got: {text}");
}

#[test]
fn type_present_but_not_string_treated_as_missing() {
    // `as_str()` on a numeric type yields None → "missing 'type'".
    let args = json!({ "title": "x", "blocks": [{ "type": 7, "data": {} }] });
    let text = handle_ui_render(&args)[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("missing 'type'"), "got: {text}");
}

#[test]
fn blocks_not_an_array_is_rejected() {
    let text = handle_ui_render(&json!({ "title": "x", "blocks": "nope" }))[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("non-empty 'blocks'"), "got: {text}");
}

// ── dispatch_tool ────────────────────────────────────────────────────────────

#[test]
fn dispatch_ui_render_missing_arguments_key_falls_to_empty() {
    // No "arguments" → args defaults to {} → blocks missing → validation error.
    let out = dispatch_tool(Some(json!({ "name": "ui_render" })));
    assert!(out[0]["text"]
        .as_str()
        .unwrap()
        .contains("non-empty 'blocks'"));
}

#[test]
fn dispatch_empty_object_is_unknown_tool() {
    let out = dispatch_tool(Some(json!({})));
    assert!(out[0]["text"].as_str().unwrap().contains("Unknown tool: "));
}

#[test]
fn dispatch_name_not_string_is_unknown_tool() {
    let out = dispatch_tool(Some(json!({ "name": 123 })));
    assert!(out[0]["text"].as_str().unwrap().contains("Unknown tool"));
}

// ── request deserialization ──────────────────────────────────────────────────

#[test]
fn mcp_request_deserializes_with_and_without_params() {
    let with: McpRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"tools/call","params":{"a":1},"id":5}"#)
            .unwrap();
    assert_eq!(with.method, "tools/call");
    assert!(with.params.is_some());
    assert_eq!(with.id, json!(5));

    let without: McpRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"initialize","id":"s"}"#).unwrap();
    assert!(without.params.is_none());
    assert_eq!(without.id, json!("s"));
}

// ── tool_definitions schema details ──────────────────────────────────────────

#[test]
fn schema_declares_operation_enum_and_block_type_desc() {
    let defs = tool_definitions();
    let schema = &defs[0]["inputSchema"];
    let op_enum: Vec<&str> = schema["properties"]["operation"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(op_enum, vec!["replace", "append", "update"]);

    // The block "type" description enumerates the documented vocabulary.
    let block_type_desc = schema["properties"]["blocks"]["items"]["properties"]["type"]
        ["description"]
        .as_str()
        .unwrap();
    for ty in ["card", "table", "mermaid", "stat-group"] {
        assert!(block_type_desc.contains(ty), "schema missing {ty}");
    }

    let data_desc = schema["properties"]["blocks"]["items"]["properties"]["data"]["description"]
        .as_str()
        .unwrap();
    assert!(data_desc.contains("alert") && data_desc.contains("message"));

    // Block items require both type and data.
    let item_req: Vec<&str> = schema["properties"]["blocks"]["items"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(item_req.contains(&"type") && item_req.contains(&"data"));
}

#[test]
fn route_tools_call_invalid_render_surfaces_validation_error() {
    let v = route_request(
        "tools/call",
        Some(json!({ "name": "ui_render", "arguments": { "title": "z", "blocks": [] } })),
        json!(11),
    )
    .unwrap();
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("non-empty 'blocks'"), "got: {text}");
}
