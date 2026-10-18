use super::*;
use crate::doctor::report::Status;
use crate::mcp_probe::{ServerInfo, ToolDef};

fn tool(name: &str) -> ToolDef {
    serde_json::from_value(serde_json::json!({ "name": name })).expect("tool")
}

#[test]
fn a_listing_reports_the_tool_count_and_names() {
    let catalog = Catalog::Listed {
        server: Some(ServerInfo {
            name: "time".into(),
            version: Some("1.0".into()),
        }),
        tools: vec![tool("now"), tool("zone")],
    };
    let check = describe("time", catalog);
    assert_eq!(check.status, Status::Pass);
    assert_eq!(check.title, "time: 2 tools");
    assert_eq!(check.detail.as_deref(), Some("time 1.0\nnow, zone"));
}

#[test]
fn a_failed_probe_keeps_its_exact_reason() {
    let check = describe(
        "x",
        Catalog::Failed {
            reason: "spawn: No such file".into(),
        },
    );
    assert_eq!(check.status, Status::Fail);
    assert_eq!(check.detail.as_deref(), Some("spawn: No such file"));
}

#[test]
fn an_unlaunchable_server_is_a_warning() {
    let check = describe("x", Catalog::Unavailable { reason: "unmet" });
    assert_eq!(check.status, Status::Warn);
}

#[test]
fn an_empty_listing_is_a_warning() {
    let catalog = Catalog::Listed {
        server: None,
        tools: Vec::new(),
    };
    assert_eq!(describe("x", catalog).status, Status::Warn);
}
