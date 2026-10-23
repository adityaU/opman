//! Preserve real call records through the ACP adapter; never infer calls from code.
use super::*;

#[test]
fn structured_mcp_records_survive_plain_and_result_envelopes() {
    let records = json!({"version":1,"calls":[]});
    for raw in [
        json!({"content":[{"type":"text","text":"done"}],"structuredContent":records}),
        json!({"result":{"content":[{"type":"text","text":"done"}],"structuredContent":records}}),
    ] {
        let mut part = json!({"tool":"mcp__node_repl__js","state":{"input":{},"status":"running"}});
        merge(&mut part, &json!({"rawOutput":raw,"status":"completed"}));
        assert_eq!(part["state"]["metadata"]["structuredContent"], records);
        assert_eq!(part["state"]["output"], "done");
    }
}

#[test]
fn explicit_parent_links_survive_delta_updates() {
    for source in [
        json!({"parentToolCallId":"outer"}),
        json!({"_meta":{"parent_tool_call_id":"outer"}}),
        json!({"parentCallID":"outer"}),
    ] {
        let mut part = json!({"tool":"read_file","state":{"input":{},"status":"running"}});
        merge(&mut part, &source);
        merge(
            &mut part,
            &json!({"status":"completed","rawOutput":"contents"}),
        );
        assert_eq!(part["state"]["metadata"]["parentCallID"], "outer");
    }
}

#[test]
fn tool_source_does_not_create_child_metadata() {
    let mut part = json!({"tool":"mcp__node_repl__js","state":{"input":{},"status":"running"}});
    merge(
        &mut part,
        &json!({"rawInput":{"code":"await tools.read_file({path:'a.rs'})"}}),
    );
    assert!(part["state"].get("metadata").is_none());
}
