use super::*;

fn operation(name: &str, args: Value) -> Value {
    to_operation(name, &args).expect("tool is known")
}

/// The wire shape is project-addressed: a pane id is a workspace detail an agent has no
/// way to learn, so no tool may name one.
#[test]
fn no_tool_asks_the_agent_for_a_pane_id() {
    let text = definitions().to_string();
    assert!(!text.contains("pane_id"), "tools must address a project, not a pane");
}

#[test]
fn every_declared_tool_maps_to_an_operation() {
    let definitions = definitions();
    let tools = definitions
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools is an array");
    assert!(!tools.is_empty());

    for tool in tools {
        let name = tool.get("name").and_then(Value::as_str).expect("named");
        assert!(
            to_operation(name, &json!({ "direction": "back" })).is_some(),
            "{name} is declared but has no operation mapping"
        );
    }
}

#[test]
fn missing_optional_arguments_are_dropped_not_sent_as_null() {
    // A null would fail the server's typed deserialiser; an absent key defaults.
    let body = operation("browser_snapshot", json!({}));
    assert_eq!(body.get("op").and_then(Value::as_str), Some("snapshot"));
    assert!(body.get("max_nodes").is_none());
    assert!(body.get("viewport_only").is_none());
}

#[test]
fn supplied_options_are_forwarded() {
    let body = operation(
        "browser_snapshot",
        json!({ "max_nodes": 50, "viewport_only": true }),
    );
    assert_eq!(body.get("max_nodes").and_then(Value::as_u64), Some(50));
    assert_eq!(body.get("viewport_only").and_then(Value::as_bool), Some(true));
}

#[test]
fn navigate_direction_becomes_the_operation_tag() {
    for direction in ["back", "forward", "reload"] {
        let body = operation(
            "browser_navigate",
            json!({ "direction": direction }),
        );
        assert_eq!(body.get("op").and_then(Value::as_str), Some(direction));
    }
}

#[test]
fn navigate_without_a_direction_reloads_rather_than_failing() {
    let body = operation("browser_navigate", json!({}));
    assert_eq!(body.get("op").and_then(Value::as_str), Some("reload"));
}

#[test]
fn listing_panes_needs_no_pane() {
    let body = operation("browser_list_panes", json!({}));
    assert_eq!(body, json!({ "op": "list" }));
}

#[test]
fn unknown_tools_are_reported_rather_than_guessed() {
    assert!(to_operation("browser_eval", &json!({})).is_none());
}

#[test]
fn a_snapshot_renders_as_the_outline_itself() {
    let value = json!({
        "url": "https://example.com/",
        "title": "Example",
        "outline": "main\n link \"Docs\" [ref=e0] →/docs",
        "truncated": false,
    });
    let text = render("browser_snapshot", &value);
    assert!(text.contains("Example"));
    assert!(text.contains("https://example.com/"));
    assert!(text.contains("[ref=e0]"));
    // The outline is not re-wrapped in JSON — indentation is the structure.
    assert!(!text.contains('{'));
}

#[test]
fn a_truncated_outline_says_how_to_get_the_rest() {
    let value = json!({ "url": "u", "title": "t", "outline": "x", "truncated": true });
    assert!(render("browser_snapshot", &value).contains("max_nodes"));
}

#[test]
fn an_empty_pane_list_explains_what_to_do_next() {
    let text = render("browser_list_panes", &json!({ "panes": [] }));
    assert!(text.contains("browser_open"), "got: {text}");
}

#[test]
fn pane_listing_is_one_line_per_pane() {
    let value = json!({
        "panes": [
            { "project": "/repo/one", "title": "Docs", "url": "https://d.example" },
            { "project": "/repo/two", "title": "App", "url": "http://localhost:3000" },
        ],
    });
    let text = render("browser_list_panes", &value);
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("/repo/one"));
    assert!(text.contains("http://localhost:3000"));
}

#[test]
fn read_text_marks_truncation() {
    let value = json!({ "url": "u", "title": "t", "text": "body", "truncated": true });
    let text = render("browser_read_text", &value);
    assert!(text.contains("body"));
    assert!(text.contains("max_chars"));
}

#[tokio::test]
async fn a_missing_web_server_is_reported_not_silently_empty() {
    let project = Project("/repo".to_string());
    let text = dispatch_tool(None, &project, Some(json!({ "name": "browser_list_panes" }))).await;
    assert!(text.contains("not running"), "got: {text}");
}
