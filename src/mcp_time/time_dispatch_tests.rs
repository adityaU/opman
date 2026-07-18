//! Wave-3 breadth coverage for the time MCP `mod.rs`: `dispatch_tool` for every
//! tool with valid/invalid/edge arguments, `route_request` permutations, the
//! tool-definition schema, and request deserialization.
use super::*;
use serde_json::json;

fn text_of(v: &serde_json::Value) -> String {
    v[0]["text"].as_str().unwrap().to_string()
}

// ── time_now ─────────────────────────────────────────────────────────────────

#[test]
fn dispatch_time_now_default_reports_system_default() {
    let out = dispatch_tool(Some(json!({ "name": "time_now", "arguments": {} })));
    let t = text_of(&out);
    assert!(t.contains("Current time"));
    assert!(t.contains("system default"));
}

#[test]
fn dispatch_time_now_explicit_valid_zone() {
    let out = dispatch_tool(Some(json!({
        "name": "time_now",
        "arguments": { "timezone": "Asia/Kolkata" }
    })));
    let t = text_of(&out);
    assert!(t.contains("Current time in Asia/Kolkata"), "got: {t}");
    assert!(t.contains("System timezone"));
}

#[test]
fn dispatch_time_now_invalid_zone_is_reported() {
    let out = dispatch_tool(Some(json!({
        "name": "time_now",
        "arguments": { "timezone": "Mars/Olympus" }
    })));
    let t = text_of(&out);
    assert!(t.contains("Unknown timezone"), "got: {t}");
}

#[test]
fn dispatch_time_now_local_keyword_uses_system() {
    let out = dispatch_tool(Some(json!({
        "name": "time_now",
        "arguments": { "timezone": "local" }
    })));
    assert!(text_of(&out).contains("system default"));
}

#[test]
fn dispatch_time_now_empty_string_zone_uses_system() {
    let out = dispatch_tool(Some(json!({
        "name": "time_now",
        "arguments": { "timezone": "" }
    })));
    assert!(text_of(&out).contains("system default"));
}

// ── time_convert ─────────────────────────────────────────────────────────────

#[test]
fn dispatch_time_convert_utc_to_kolkata() {
    let out = dispatch_tool(Some(json!({
        "name": "time_convert",
        "arguments": {
            "datetime": "2024-01-15 10:00:00",
            "from_timezone": "UTC",
            "to_timezone": "Asia/Kolkata"
        }
    })));
    let t = text_of(&out);
    assert!(t.contains("→"), "expected a conversion arrow: {t}");
}

#[test]
fn dispatch_time_convert_invalid_datetime() {
    let out = dispatch_tool(Some(json!({
        "name": "time_convert",
        "arguments": {
            "datetime": "not-a-date",
            "from_timezone": "UTC",
            "to_timezone": "UTC"
        }
    })));
    // Any non-panicking string response is acceptable; parse failure surfaces text.
    assert!(!text_of(&out).is_empty());
}

#[test]
fn dispatch_time_convert_missing_args_does_not_panic() {
    let out = dispatch_tool(Some(json!({ "name": "time_convert", "arguments": {} })));
    assert_eq!(out[0]["type"], "text");
}

// ── time_zones ───────────────────────────────────────────────────────────────

#[test]
fn dispatch_time_zones_no_search_lists_many() {
    let out = dispatch_tool(Some(json!({ "name": "time_zones", "arguments": {} })));
    assert!(text_of(&out).to_lowercase().contains("timezone"));
}

#[test]
fn dispatch_time_zones_search_filters() {
    let out = dispatch_tool(Some(json!({
        "name": "time_zones",
        "arguments": { "search": "kolkata" }
    })));
    assert!(text_of(&out).to_lowercase().contains("kolkata"));
}

// ── dispatch edge cases ──────────────────────────────────────────────────────

#[test]
fn dispatch_unknown_tool_named() {
    let out = dispatch_tool(Some(json!({ "name": "time_teleport", "arguments": {} })));
    assert!(text_of(&out).contains("Unknown tool: time_teleport"));
}

#[test]
fn dispatch_missing_arguments_defaults_empty() {
    // No "arguments" key → args defaults to {} → time_now with system default.
    let out = dispatch_tool(Some(json!({ "name": "time_now" })));
    assert!(text_of(&out).contains("Current time"));
}

#[test]
fn dispatch_name_not_string_unknown() {
    let out = dispatch_tool(Some(json!({ "name": 5 })));
    assert!(text_of(&out).contains("Unknown tool"));
}

// ── route + schema ───────────────────────────────────────────────────────────

#[test]
fn route_tools_call_time_convert() {
    let params = Some(json!({
        "name": "time_convert",
        "arguments": { "datetime": "2024-01-15 10:00", "from_timezone": "UTC", "to_timezone": "UTC" }
    }));
    let v = route_request("tools/call", params, json!(3)).unwrap();
    assert_eq!(v["result"]["content"][0]["type"], "text");
}

#[test]
fn schema_time_now_and_zones_have_no_required() {
    let defs = tool_definitions();
    let arr = defs.as_array().unwrap();
    for name in ["time_now", "time_zones"] {
        let t = arr.iter().find(|t| t["name"] == name).unwrap();
        // These tools take only optional properties → no "required" key.
        assert!(t["inputSchema"].get("required").is_none(), "{name} unexpectedly required");
    }
}

#[test]
fn schema_time_convert_property_descriptions_present() {
    let defs = tool_definitions();
    let convert = defs
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "time_convert")
        .unwrap();
    let props = &convert["inputSchema"]["properties"];
    assert!(props["datetime"]["description"].is_string());
    assert!(props["from_timezone"]["description"].is_string());
    assert!(props["to_timezone"]["description"].is_string());
}

#[test]
fn mcp_request_deserializes_with_params() {
    let r: McpRequest = serde_json::from_str(
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"time_now"},"id":8}"#,
    )
    .unwrap();
    assert_eq!(r.method, "tools/call");
    assert!(r.params.is_some());
    assert_eq!(r.id, json!(8));
}

#[test]
fn handle_line_routes_tools_call() {
    let line = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"time_zones","arguments":{"search":"utc"}},"id":2}"#;
    let resp = handle_line(line).unwrap();
    assert_eq!(resp["result"]["content"][0]["type"], "text");
}
