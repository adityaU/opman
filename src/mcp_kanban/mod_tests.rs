use super::*;
use serde_json::json;

#[test]
fn tool_definitions_has_seven_tools() {
    let defs = tool_definitions();
    let arr = defs.as_array().unwrap();
    assert_eq!(arr.len(), 7);
    let names: Vec<&str> = arr.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "kanban_get_task",
        "kanban_set_lane",
        "kanban_add_note",
        "kanban_complete",
        "kanban_list_tasks",
        "kanban_board_summary",
        "kanban_read_notes",
    ] {
        assert!(names.contains(&expected), "missing {expected}");
    }
}

#[test]
fn every_tool_requires_task_id() {
    for t in tool_definitions().as_array().unwrap() {
        let req = t["inputSchema"]["required"].as_array().unwrap();
        assert!(
            req.iter().any(|v| v == "task_id"),
            "{} lacks task_id",
            t["name"]
        );
    }
}

#[test]
fn set_lane_requires_lane() {
    let defs = tool_definitions();
    let sl = defs
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "kanban_set_lane")
        .unwrap();
    let req: Vec<&str> = sl["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(req.contains(&"lane"));
}

#[tokio::test]
async fn route_initialize() {
    let v = route_request(None, "initialize", None, json!(1))
        .await
        .unwrap();
    assert_eq!(v["result"]["serverInfo"]["name"], "opman-kanban");
}

#[tokio::test]
async fn route_notifications_none() {
    assert!(route_request(None, "notifications/initialized", None, json!(0))
        .await
        .is_none());
}

#[tokio::test]
async fn route_tools_list() {
    let v = route_request(None, "tools/list", None, json!(2))
        .await
        .unwrap();
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 7);
}

#[tokio::test]
async fn route_tools_call_without_internal() {
    let params = Some(json!({"name":"kanban_get_task","arguments":{"task_id":"tsk_1"}}));
    let v = route_request(None, "tools/call", params, json!(3))
        .await
        .unwrap();
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Kanban API is unavailable"));
}

#[tokio::test]
async fn route_unknown_method() {
    let v = route_request(None, "frobnicate", None, json!(4))
        .await
        .unwrap();
    assert_eq!(v["error"]["code"], -32601);
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("frobnicate"));
}

#[test]
fn load_internal_does_not_panic() {
    // Reads ~/.config/opman/internal.json if present; returns None otherwise.
    // Either outcome is acceptable — we only assert it does not panic.
    let _ = load_internal();
}

#[tokio::test]
async fn write_response_happy_path() {
    let mut stdout = tokio::io::stdout();
    write_response(&mut stdout, &json!({"ok":true})).await;
}
