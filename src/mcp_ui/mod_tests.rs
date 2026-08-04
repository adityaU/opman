use super::*;
use serde_json::json;

#[test]
fn tool_definitions_single_ui_render() {
    let defs = tool_definitions();
    let arr = defs.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "ui_render");
    let req: Vec<&str> = arr[0]["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(req.contains(&"title") && req.contains(&"blocks"));
}

#[test]
fn handle_ui_render_valid() {
    let args = json!({
        "title":"Build",
        "blocks":[{"type":"card","data":{"title":"x"}}]
    });
    let out = handle_ui_render(&args);
    let text = out[0]["text"].as_str().unwrap();
    assert!(text.contains("Rendered UI: Build (1 blocks)"));
}

#[test]
fn handle_ui_render_with_delta_operation() {
    let args = json!({
        "title":"Progress",
        "blocks":[{"type":"steps","data":{}}],
        "render_id":"r1",
        "operation":"update"
    });
    let out = handle_ui_render(&args);
    let text = out[0]["text"].as_str().unwrap();
    assert!(text.contains("update:r1"));
}

#[test]
fn handle_ui_render_default_title() {
    let args = json!({ "blocks":[{"type":"card","data":{}}] });
    let out = handle_ui_render(&args);
    assert!(out[0]["text"].as_str().unwrap().contains("Rendered UI: UI"));
}

#[test]
fn handle_ui_render_missing_blocks() {
    let out = handle_ui_render(&json!({"title":"x"}));
    assert!(out[0]["text"]
        .as_str()
        .unwrap()
        .contains("non-empty 'blocks'"));
}

#[test]
fn handle_ui_render_empty_blocks() {
    let out = handle_ui_render(&json!({"title":"x","blocks":[]}));
    assert!(out[0]["text"]
        .as_str()
        .unwrap()
        .contains("non-empty 'blocks'"));
}

#[test]
fn handle_ui_render_block_missing_type() {
    let out = handle_ui_render(&json!({"title":"x","blocks":[{"data":{}}]}));
    assert!(out[0]["text"].as_str().unwrap().contains("missing 'type'"));
}

#[test]
fn handle_ui_render_block_missing_data() {
    let out = handle_ui_render(&json!({"title":"x","blocks":[{"type":"card"}]}));
    assert!(out[0]["text"].as_str().unwrap().contains("missing 'data'"));
}

#[test]
fn dispatch_ui_render() {
    let out = dispatch_tool(Some(json!({
        "name":"ui_render",
        "arguments":{"title":"T","blocks":[{"type":"card","data":{}}]}
    })));
    assert!(out[0]["text"].as_str().unwrap().contains("Rendered UI"));
}

#[test]
fn dispatch_unknown_tool() {
    let out = dispatch_tool(Some(json!({"name":"other"})));
    assert!(out[0]["text"]
        .as_str()
        .unwrap()
        .contains("Unknown tool: other"));
}

#[test]
fn dispatch_none_params() {
    let out = dispatch_tool(None);
    assert!(out[0]["text"].as_str().unwrap().contains("Unknown tool"));
}

#[test]
fn route_initialize() {
    let v = route_request("initialize", None, json!(1)).unwrap();
    assert_eq!(v["result"]["serverInfo"]["name"], "opman-ui");
    assert_eq!(v["result"]["serverInfo"]["version"], "1.1.0");
}

#[test]
fn route_notifications_none() {
    assert!(route_request("notifications/initialized", None, json!(0)).is_none());
}

#[test]
fn route_tools_list() {
    let v = route_request("tools/list", None, json!(2)).unwrap();
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 1);
}

#[test]
fn route_tools_call() {
    let params = Some(json!({
        "name":"ui_render",
        "arguments":{"title":"T","blocks":[{"type":"card","data":{}}]}
    }));
    let v = route_request("tools/call", params, json!(3)).unwrap();
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Rendered UI"));
}

#[test]
fn route_unknown_method() {
    let v = route_request("bogus", None, json!(4)).unwrap();
    assert_eq!(v["error"]["code"], -32601);
}

#[tokio::test]
async fn write_response_happy_path() {
    let mut stdout = tokio::io::stdout();
    write_response(&mut stdout, &json!({"ok":true})).await;
}
