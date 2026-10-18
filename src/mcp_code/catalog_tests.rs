use std::sync::Arc;

use super::*;
use crate::mcp_code::names::Namespace;
use crate::mcp_code::testing::tool_defs;

pub fn sample() -> Catalog {
    let tools: Vec<crate::mcp_probe::ToolDef> = serde_json::from_value(tool_defs()).expect("defs");
    Catalog {
        groups: vec![Group {
            namespace: Namespace::for_server("fake", &Default::default()),
            tools: Arc::from(tools),
        }],
        unavailable: Vec::new(),
    }
}

#[test]
fn finds_groups_and_tools() {
    let catalog = sample();
    assert!(catalog.group("fake").is_some());
    assert!(catalog.group("nope").is_none());
    assert!(catalog.tool("fake", "add").is_some());
    assert!(catalog.tool("fake", "nope").is_none());
    assert_eq!(catalog.tool_count(), 6);
}
