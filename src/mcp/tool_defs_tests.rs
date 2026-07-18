use super::*;

fn names() -> Vec<String> {
    mcp_tool_definitions()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn defines_seven_tools() {
    let defs = mcp_tool_definitions();
    assert_eq!(defs.as_array().unwrap().len(), 7);
}

#[test]
fn expected_tool_names_present() {
    let n = names();
    for expected in [
        "terminal_read",
        "terminal_run",
        "terminal_list",
        "terminal_new",
        "terminal_close",
        "terminal_rename",
        "terminal_ephemeral_run",
    ] {
        assert!(n.iter().any(|x| x == expected), "missing {expected}");
    }
}

#[test]
fn every_tool_has_description_and_object_schema() {
    for t in mcp_tool_definitions().as_array().unwrap() {
        assert!(t["description"].as_str().is_some());
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(t["inputSchema"]["properties"].is_object());
    }
}

#[test]
fn required_fields_are_correct() {
    let defs = mcp_tool_definitions();
    let by_name = |name: &str| {
        defs.as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()
            .clone()
    };

    let run = by_name("terminal_run");
    let req: Vec<&str> = run["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(req.contains(&"command") && req.contains(&"tab"));

    let rename = by_name("terminal_rename");
    let rreq: Vec<&str> = rename["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(rreq.contains(&"tab") && rreq.contains(&"name"));

    let eph = by_name("terminal_ephemeral_run");
    let ereq: Vec<&str> = eph["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(ereq.contains(&"command") && ereq.contains(&"name"));

    // terminal_list takes no properties / no required.
    let list = by_name("terminal_list");
    assert!(list["inputSchema"]["required"].is_null());
}
