//! The registry itself, and the one test that justifies the module existing: the same
//! server set rendering into all four runner shapes.

use super::*;
use crate::mcp_registry::config::{McpConfig, ServerConfig};
use crate::mcp_registry::spec::Arg;

fn user(raw: &str) -> McpConfig {
    serde_json::from_str(raw).expect("valid test config")
}

#[test]
fn builtins_load_when_the_user_has_no_config() {
    let registry = McpRegistry::from_config(BuiltinFlags::ALL, McpConfig::default());
    let all = RunnerKind::Codex;
    let names: Vec<_> = registry.for_runner(&all).map(ServerSpec::name).collect();
    assert!(names.contains(&"terminal"));
    assert!(names.contains(&"time"));
}

#[test]
fn a_bare_toggle_disables_a_builtin_without_restating_it() {
    let registry = McpRegistry::from_config(
        BuiltinFlags::ALL,
        user(r#"{"servers":{"time":{"enabled":false}}}"#),
    );
    assert!(registry.get("time").is_none());
    assert!(registry.get("terminal").is_some());
}

#[test]
fn a_bare_enable_leaves_the_builtin_launch_command_intact() {
    let registry = McpRegistry::from_config(
        BuiltinFlags::ALL,
        user(r#"{"servers":{"time":{"enabled":true}}}"#),
    );
    let time = registry.get("time").expect("time survives");
    assert!(time.binds_session() == false);
    let Transport::Stdio(stdio) = time.transport() else {
        panic!("expected stdio");
    };
    assert_eq!(stdio.args[0], Arg::lit("mcp-time"));
}

#[test]
fn a_user_entry_with_a_new_name_is_added() {
    let registry = McpRegistry::from_config(
        BuiltinFlags::default(),
        user(r#"{"servers":{"linear":{"url":"https://mcp.linear.app/mcp","auth":"oauth"}}}"#),
    );
    assert!(registry.get("linear").is_some());
}

#[test]
fn runner_scoping_is_honoured() {
    let registry = McpRegistry::from_config(
        BuiltinFlags::default(),
        user(r#"{"servers":{"only-codex":{"command":"x","runners":["codex"]}}}"#),
    );
    let in_codex: Vec<_> = registry
        .for_runner(&RunnerKind::Codex)
        .map(ServerSpec::name)
        .collect();
    let in_opencode: Vec<_> = registry
        .for_runner(&RunnerKind::Opencode)
        .map(ServerSpec::name)
        .collect();
    assert!(in_codex.contains(&"only-codex"));
    assert!(!in_opencode.contains(&"only-codex"));
}

#[test]
fn binds_session_is_false_when_no_offered_server_needs_one() {
    let registry = McpRegistry::from_specs(
        vec![ServerSpec::stdio(
            "time",
            "/opman",
            vec![Arg::lit("mcp-time")],
            Vec::new(),
        )],
        BuiltinFlags::default(),
    );
    assert!(!registry.binds_session(&RunnerKind::Codex));
}

#[test]
fn binds_session_is_true_when_one_does() {
    let registry = McpRegistry::from_specs(
        vec![ServerSpec::stdio(
            "t",
            "/opman",
            vec![Arg::lit("mcp")],
            vec![("S".into(), Arg::SessionId)],
        )],
        BuiltinFlags::default(),
    );
    assert!(registry.binds_session(&RunnerKind::Codex));
}

/// The regression guard for "one registry, four shapes". Nothing like it existed while
/// the four injection sites were hand-rolled, which is exactly why they drifted.
#[test]
fn one_registry_renders_every_runner_with_the_same_server_set() {
    let registry = McpRegistry::from_specs(
        vec![
            ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new()),
            ServerSpec::stdio("ext", "npx", vec![Arg::lit("-y"), Arg::lit("pkg")], Vec::new()),
        ],
        BuiltinFlags::default(),
    );
    let at = Bind::new("/opman", "/proj", Some("ses1"));

    let claude = render::claude_mcp_config(registry.for_runner(&RunnerKind::ClaudeCode), at)
        .expect("claude payload");
    let claude: serde_json::Value = serde_json::from_str(&claude).expect("claude json");
    let codex = render::codex_thread_config(registry.for_runner(&RunnerKind::Codex), at);
    let acp = render::acp_servers(
        registry.for_runner(&RunnerKind::Acp("claude".into())),
        at,
        RemoteCaps::STDIO_ONLY,
    );
    let opencode = render::opencode_config(
        registry.for_runner(&RunnerKind::Opencode),
        at,
        BuiltinFlags::default(),
    )
    .expect("opencode payload");
    let opencode: serde_json::Value = serde_json::from_str(&opencode).expect("opencode json");

    let mut claude_names: Vec<_> = claude["mcpServers"]
        .as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect();
    let mut codex_names: Vec<_> = codex["mcp_servers"]
        .as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect();
    let mut acp_names: Vec<_> = acp
        .as_array()
        .expect("array")
        .iter()
        .map(|entry| entry["name"].as_str().unwrap_or_default().to_string())
        .collect();
    let mut opencode_names: Vec<_> = opencode["mcp"]
        .as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect();
    for names in [
        &mut claude_names,
        &mut codex_names,
        &mut acp_names,
        &mut opencode_names,
    ] {
        names.sort();
    }
    assert_eq!(claude_names, ["ext", "time"]);
    assert_eq!(codex_names, claude_names);
    assert_eq!(acp_names, claude_names);
    assert_eq!(opencode_names, claude_names);
}

use crate::mcp_registry::spec::Transport;
