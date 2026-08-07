//! ACP's array shape. The name/value pair form for env and headers is what existing
//! agents accept, so these assertions are load-bearing.

use super::*;
use crate::mcp_registry::spec::{Arg, Auth, Presence, Remote, RunnerScope, Transport};

fn at<'a>() -> Bind<'a> {
    Bind::new("/opman", "/proj", Some("ses1"))
}

fn remote(kind: RemoteKind) -> ServerSpec {
    ServerSpec {
        name: "r".into(),
        transport: Transport::Remote(Remote {
            kind,
            url: "https://x/mcp".into(),
            headers: Box::new([]),
            auth: Auth::None,
        }),
        presence: Presence::Always,
        scope: RunnerScope::default(),
        timeout_secs: None,
    }
}

#[test]
fn the_result_is_an_array_of_named_entries() {
    let spec = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    let json = servers([&spec], at(), RemoteCaps::STDIO_ONLY);
    assert_eq!(json[0]["name"], "time");
    assert_eq!(json[0]["command"], "/opman");
    assert_eq!(json[0]["args"][0], "mcp-time");
}

#[test]
fn env_is_a_name_value_pair_list_not_an_object() {
    let spec = ServerSpec::stdio(
        "terminal",
        "/opman",
        vec![Arg::lit("mcp"), Arg::Dir],
        vec![("OPENCODE_SESSION_ID".into(), Arg::SessionId)],
    );
    let json = servers([&spec], at(), RemoteCaps::STDIO_ONLY);
    assert_eq!(json[0]["env"][0]["name"], "OPENCODE_SESSION_ID");
    assert_eq!(json[0]["env"][0]["value"], "ses1");
}

#[test]
fn stdio_entries_carry_no_type_field() {
    let spec = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    let json = servers([&spec], at(), RemoteCaps::STDIO_ONLY);
    assert!(json[0].get("type").is_none());
}

#[test]
fn a_remote_reaches_an_agent_that_advertised_the_transport() {
    let json = servers([&remote(RemoteKind::Http)], at(), RemoteCaps::new(true, false));
    assert_eq!(json[0]["type"], "http");
    assert_eq!(json[0]["url"], "https://x/mcp");
    assert!(json[0]["headers"].is_array());
}

#[test]
fn a_remote_is_proxied_for_an_agent_that_did_not() {
    // Absent mcpCapabilities means unsupported, which is the safe reading for an agent
    // that predates the field.
    let json = servers([&remote(RemoteKind::Http)], at(), RemoteCaps::STDIO_ONLY);
    assert_eq!(json[0]["args"][0], "mcp-proxy");
    assert!(json[0].get("url").is_none());
}

#[test]
fn an_sse_remote_needs_the_sse_capability_specifically() {
    let http_only = servers([&remote(RemoteKind::Sse)], at(), RemoteCaps::new(true, false));
    assert_eq!(http_only[0]["args"][0], "mcp-proxy");
    let sse_ok = servers([&remote(RemoteKind::Sse)], at(), RemoteCaps::new(false, true));
    assert_eq!(sse_ok[0]["type"], "sse");
}

#[test]
fn an_empty_set_is_an_empty_array() {
    let json = servers(std::iter::empty(), at(), RemoteCaps::STDIO_ONLY);
    assert_eq!(json, serde_json::json!([]));
}
