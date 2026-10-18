use super::*;
use crate::mcp_registry::config::ServerConfig;
use crate::mcp_registry::spec::{Arg, ServerSpec};
use crate::mcp_registry::{McpRegistry};

fn stdio(name: &str) -> ServerSpec {
    ServerSpec::stdio(name, "/bin/true", vec![Arg::lit("x")], Vec::new())
}

#[test]
fn the_exclusion_list_is_explicit() {
    for name in ["code", "ask", "ui", "agent-manager", "kanban"] {
        assert!(Excluded::names(name), "{name}");
    }
    for name in ["time", "terminal", "opman-tools", "skills", "browser"] {
        assert!(!Excluded::names(name), "{name}");
    }
}

#[test]
fn excluded_and_hidden_servers_are_not_mounted() {
    let hidden = ServerConfig {
        code_mode: Some(false),
        ..ServerConfig::default()
    }
    .patch(stdio("secret"))
    .expect("patched");
    let registry = McpRegistry::from_specs(
        vec![
            stdio("time"),
            stdio("ask"),
            stdio("code"),
            stdio("opman-tools"),
            hidden,
        ],
    );
    let pool = pool_from_registry(&registry, "/work", None);
    assert_eq!(pool.namespaces(), ["time", "opman_tools"]);
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
}

#[test]
fn colliding_names_get_distinct_namespaces() {
    let registry =
        McpRegistry::from_specs(vec![stdio("a-b"), stdio("a_b")]);
    let pool = pool_from_registry(&registry, "/work", None);
    assert_eq!(pool.namespaces(), ["a_b", "a_b_2"]);
}
