use super::*;
use crate::mcp_registry::config::ServerConfig;
use crate::mcp_registry::spec::{Arg, ServerSpec};
use crate::mcp_registry::McpRegistry;

fn stdio(name: &str) -> ServerSpec {
    ServerSpec::stdio(name, "/bin/true", vec![Arg::lit("x")], Vec::new())
}

#[test]
fn only_code_is_excluded_to_prevent_recursion() {
    for name in ["code"] {
        assert!(Excluded::names(name), "{name}");
    }
    for name in [
        "ask",
        "ui",
        "agent-manager",
        "kanban",
        "time",
        "terminal",
        "opman-tools",
        "skills",
        "browser",
    ] {
        assert!(!Excluded::names(name), "{name}");
    }
}

#[test]
fn code_mode_false_server_is_mounted_for_live_settings() {
    let hidden = ServerConfig {
        code_mode: Some(false),
        ..ServerConfig::default()
    }
    .patch(stdio("secret"))
    .expect("patched");
    let registry = McpRegistry::from_specs(vec![
        stdio("ask"),
        stdio("time"),
        stdio("code"),
        stdio("opman-tools"),
        hidden,
    ]);
    let pool = pool_from_registry(&registry, "/work", None);
    assert_eq!(pool.namespaces(), ["ask", "time", "opman_tools", "secret"]);
}

#[test]
fn remote_servers_are_mounted_through_the_proxy() {
    let remote = ServerConfig {
        url: "https://example.com/mcp".into(),
        ..ServerConfig::default()
    }
    .to_spec("my-remote")
    .expect("spec");
    let registry = McpRegistry::from_specs(vec![remote]);
    let pool = pool_from_registry(&registry, "/work", None);
    assert_eq!(pool.namespaces(), ["my_remote"]);

    let spec = registry.get("my-remote").expect("remote spec");
    let wire = registry
        .stdio_launch("my-remote", registry.bind("/work", None))
        .expect("proxy launch");
    let launch = code_mode_launch(spec, &wire);
    assert!(launch
        .env
        .iter()
        .any(|(name, value)| { name == crate::mcp_proxy::CODE_MODE_PROXY_ENV && value == "1" }));
}

#[test]
fn colliding_names_get_distinct_namespaces() {
    let registry = McpRegistry::from_specs(vec![stdio("a-b"), stdio("a_b")]);
    let pool = pool_from_registry(&registry, "/work", None);
    assert_eq!(pool.namespaces(), ["a_b", "a_b_2"]);
}
