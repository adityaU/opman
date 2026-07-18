//! Generated tests for the MCP tool definitions.

use super::*;

#[test]
fn definitions_are_an_array_of_eight_tools() {
    let defs = web_mcp_tool_definitions();
    let arr = defs.as_array().expect("tool definitions is an array");
    assert_eq!(arr.len(), 8);
}

#[test]
fn definitions_include_all_expected_names() {
    let defs = web_mcp_tool_definitions();
    let names: Vec<&str> = defs
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in [
        "web_terminal_read",
        "web_terminal_run",
        "web_terminal_list",
        "web_terminal_new",
        "web_terminal_close",
        "web_editor_open",
        "web_editor_read",
        "web_editor_list",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
}

#[test]
fn every_tool_has_description_and_object_schema() {
    let defs = web_mcp_tool_definitions();
    for tool in defs.as_array().unwrap() {
        assert!(tool["description"].as_str().is_some_and(|d| !d.is_empty()));
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
}

#[test]
fn required_fields_are_declared_where_expected() {
    let defs = web_mcp_tool_definitions();
    let by_name = |n: &str| {
        defs.as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == n)
            .cloned()
            .unwrap()
    };
    // web_terminal_run requires id + command.
    let run = by_name("web_terminal_run");
    let req: Vec<&str> = run["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(req.contains(&"id") && req.contains(&"command"));

    // web_editor_open requires path.
    let open = by_name("web_editor_open");
    assert_eq!(open["inputSchema"]["required"][0], "path");

    // web_terminal_list has no required fields.
    let list = by_name("web_terminal_list");
    assert!(list["inputSchema"].get("required").is_none());
}
