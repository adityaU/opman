//! Engine-level tests, including a live round-trip against a real ACP server.

use super::*;

fn test_engine(agent: config::AgentConfig) -> Arc<AcpEngine> {
    engine_with_mcp(agent, crate::mcp_registry::BuiltinFlags::default())
}

fn engine_with_mcp(
    agent: config::AgentConfig,
    flags: crate::mcp_registry::BuiltinFlags,
) -> Arc<AcpEngine> {
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new("test".to_string(), agent, None, registry))
}

/// ACP reports per-turn usage; the engine this replaced had no channel for it, which is why
/// its sessions always displayed zero tokens.
#[test]
fn usage_tokens_maps_acp_field_names() {
    let tokens = usage_tokens(&json!({
        "inputTokens": 4, "outputTokens": 295,
        "cachedReadTokens": 78535, "cachedWriteTokens": 5807, "totalTokens": 84641
    }));
    assert_eq!(tokens["input"], 4);
    assert_eq!(tokens["output"], 295);
    assert_eq!(tokens["cache"]["read"], 78535);
    assert_eq!(tokens["cache"]["write"], 5807);
}

/// Missing fields must read as zero rather than dropping the whole usage report: agents are
/// free to omit any of these.
#[test]
fn usage_tokens_defaults_absent_fields_to_zero() {
    let tokens = usage_tokens(&json!({ "outputTokens": 12 }));
    assert_eq!(tokens["input"], 0);
    assert_eq!(tokens["output"], 12);
    assert_eq!(tokens["cache"]["read"], 0);
}

/// MCP injection is opt-out per agent, because an agent that cannot speak MCP should not be
/// handed a server list it will choke on.
#[test]
fn mcp_servers_are_omitted_when_injection_is_disabled() {
    let agent = config::AgentConfig {
        inject_mcp: false,
        ..Default::default()
    };
    let engine = test_engine(agent);
    assert_eq!(
        engine.mcp_servers("/tmp", "ses1", mcp_servers::McpCaps::default()),
        json!([])
    );
}

/// ACP wants `mcpServers` as a list of named stdio servers with `env` as name/value pairs —
/// a different shape from the `--mcp-config` object the CLI took.
#[test]
fn mcp_servers_use_the_acp_list_shape() {
    let agent = config::AgentConfig {
        inject_mcp: true,
        ..Default::default()
    };
    let flags = crate::mcp_registry::BuiltinFlags {
        terminal: true,
        time: true,
        ..Default::default()
    };
    let engine = engine_with_mcp(agent, flags);
    let servers = engine.mcp_servers("/tmp/project", "ses1", mcp_servers::McpCaps::default());
    let list = servers.as_array().expect("expected a list of servers");

    let time = list
        .iter()
        .find(|s| s["name"] == "time")
        .expect("time server should be present");
    assert_eq!(time["args"][0], "mcp-time");

    // env is a name/value pair list, not an object. Only the bridges that route by
    // session declare it — `time` does not read it, so it no longer carries one.
    let terminal = list
        .iter()
        .find(|s| s["name"] == "terminal")
        .expect("terminal server should be present");
    assert_eq!(terminal["args"][1], "/tmp/project");
    assert_eq!(terminal["env"][0]["name"], "OPENCODE_SESSION_ID");
    assert_eq!(terminal["env"][0]["value"], "ses1");
    assert_eq!(time["env"], json!([]));
}

// Live round-trips, nested here rather than beside the engine: they are the same tests, only
// the ones that need a real agent.
#[path = "mod_live_tests.rs"]
mod live;

#[path = "mod_live_send_tests.rs"]
mod live_send;
