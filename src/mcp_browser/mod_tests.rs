use super::*;

fn project() -> Project {
    Project("/repo".to_string())
}

async fn call(method: &str, params: Option<Value>) -> Option<Value> {
    route_request(None, &project(), method, params, json!(1)).await
}

#[tokio::test]
async fn initialize_advertises_tools() {
    let response = call("initialize", None).await.expect("initialize replies");
    let capabilities = response
        .pointer("/result/capabilities/tools")
        .expect("tools capability");
    assert!(capabilities.is_object());
    assert_eq!(
        response.pointer("/result/serverInfo/name").and_then(Value::as_str),
        Some("opman-browser")
    );
}

#[tokio::test]
async fn initialized_notification_takes_no_reply() {
    assert!(call("notifications/initialized", None).await.is_none());
}

#[tokio::test]
async fn tools_list_returns_the_schemas() {
    let response = call("tools/list", None).await.expect("a reply");
    let tools = response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("tools array");
    let names: Vec<_> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();
    assert!(names.contains(&"browser_snapshot"));
    assert!(names.contains(&"browser_click"));
    assert!(
        !names.iter().any(|name| name.contains("html")),
        "no tool should hand back raw HTML"
    );
}

#[tokio::test]
async fn an_unknown_method_is_a_protocol_error() {
    let response = call("resources/list", None).await.expect("a reply");
    assert_eq!(
        response.pointer("/error/code").and_then(Value::as_i64),
        Some(-32601)
    );
}

#[tokio::test]
async fn a_malformed_line_produces_a_parse_error_and_keeps_the_loop_alive() {
    let input = "not json\n{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"id\":2}\n";
    let mut output = Vec::new();
    run_browser_bridge(None, project(), input.as_bytes(), &mut output)
        .await
        .expect("the bridge runs to EOF");

    let text = String::from_utf8(output).expect("utf-8 output");
    let mut lines = text.lines();
    let first: Value = serde_json::from_str(lines.next().expect("a parse error")).expect("json");
    assert_eq!(first.pointer("/error/code").and_then(Value::as_i64), Some(-32700));

    // The second request was still served: one bad line must not end the session.
    let second: Value = serde_json::from_str(lines.next().expect("a second reply")).expect("json");
    assert_eq!(second.pointer("/id").and_then(Value::as_i64), Some(2));
}

#[tokio::test]
async fn blank_lines_are_ignored() {
    let mut output = Vec::new();
    run_browser_bridge(None, project(), "\n\n  \n".as_bytes(), &mut output)
        .await
        .expect("the bridge runs to EOF");
    assert!(output.is_empty());
}
