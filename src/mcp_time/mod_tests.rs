use super::*;
use serde_json::json;

#[test]
fn tool_definitions_has_three_tools() {
    let defs = tool_definitions();
    let arr = defs.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    let names: Vec<&str> = arr.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"time_now"));
    assert!(names.contains(&"time_convert"));
    assert!(names.contains(&"time_zones"));
}

#[test]
fn time_convert_required_fields() {
    let defs = tool_definitions();
    let convert = defs
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "time_convert")
        .unwrap();
    let req: Vec<&str> = convert["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(req, vec!["datetime", "from_timezone", "to_timezone"]);
}

#[test]
fn dispatch_time_now() {
    let out = dispatch_tool(Some(json!({"name":"time_now","arguments":{}})));
    assert_eq!(out[0]["type"], "text");
    assert!(out[0]["text"].as_str().unwrap().contains("Current time"));
}

#[test]
fn dispatch_time_convert() {
    let out = dispatch_tool(Some(json!({
        "name":"time_convert",
        "arguments":{"datetime":"2024-01-15 10:00:00","from_timezone":"UTC","to_timezone":"UTC"}
    })));
    assert!(out[0]["text"].as_str().unwrap().contains("→"));
}

#[test]
fn dispatch_time_zones() {
    let out = dispatch_tool(Some(
        json!({"name":"time_zones","arguments":{"search":"utc"}}),
    ));
    assert!(out[0]["text"].as_str().unwrap().contains("timezone"));
}

#[test]
fn dispatch_unknown_tool() {
    let out = dispatch_tool(Some(json!({"name":"nope"})));
    assert!(out[0]["text"]
        .as_str()
        .unwrap()
        .contains("Unknown tool: nope"));
}

#[test]
fn dispatch_none_params() {
    let out = dispatch_tool(None);
    // empty tool name → "Unknown tool: "
    assert!(out[0]["text"].as_str().unwrap().contains("Unknown tool"));
}

#[test]
fn route_initialize() {
    let v = route_request("initialize", None, json!(1)).unwrap();
    assert_eq!(v["result"]["serverInfo"]["name"], "opman-time");
    assert_eq!(v["id"], json!(1));
}

#[test]
fn route_notifications_returns_none() {
    assert!(route_request("notifications/initialized", None, json!(null)).is_none());
}

#[test]
fn route_tools_list() {
    let v = route_request("tools/list", None, json!(2)).unwrap();
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 3);
}

#[test]
fn route_tools_call() {
    let params = Some(json!({"name":"time_zones","arguments":{"search":"utc"}}));
    let v = route_request("tools/call", params, json!(3)).unwrap();
    assert_eq!(v["result"]["content"][0]["type"], "text");
}

#[test]
fn route_unknown_method() {
    let v = route_request("frob", None, json!(4)).unwrap();
    assert_eq!(v["error"]["code"], -32601);
    assert!(v["error"]["message"].as_str().unwrap().contains("frob"));
}

#[tokio::test]
async fn write_response_happy_path() {
    let mut stdout = tokio::io::stdout();
    write_response(&mut stdout, &json!({"ok":true})).await;
}
