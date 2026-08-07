//! Codex's snake_case `mcp_servers` shape.

use super::*;
use crate::mcp_registry::spec::{Arg, Auth, Presence, Remote, RemoteKind, RunnerScope, Transport};

fn at<'a>() -> Bind<'a> {
    Bind::new("/opman", "/proj", Some("ses1"))
}

#[test]
fn the_wrapper_key_is_snake_case() {
    let spec = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    let json = config([&spec], at());
    assert!(json.get("mcp_servers").is_some());
    assert_eq!(json["mcp_servers"]["time"]["args"][0], "mcp-time");
}

#[test]
fn an_empty_set_still_emits_the_wrapper() {
    // Codex takes this as a config object; omitting the key would leave stale servers.
    let json = config(std::iter::empty(), at());
    assert!(json["mcp_servers"].as_object().is_some_and(|m| m.is_empty()));
}

#[test]
fn the_timeout_uses_codex_own_key_in_seconds() {
    // Codex is the runner that most needs this: it stops at 300s and does not reset
    // that clock on progress notifications.
    let mut spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp")], Vec::new());
    spec.timeout_secs = Some(900);
    let json = config([&spec], at());
    assert_eq!(json["mcp_servers"]["x"]["tool_timeout_sec"], 900);
    assert!(json["mcp_servers"]["x"].get("timeout").is_none());
}

#[test]
fn a_remote_uses_http_headers_not_headers() {
    let spec = ServerSpec {
        name: "r".into(),
        transport: Transport::Remote(Remote {
            kind: RemoteKind::Http,
            url: "https://x/mcp".into(),
            headers: Box::new([]),
            auth: Auth::None,
        }),
        presence: Presence::Always,
        scope: RunnerScope::default(),
        timeout_secs: None,
    };
    let json = config([&spec], at());
    assert_eq!(json["mcp_servers"]["r"]["url"], "https://x/mcp");
    assert!(json["mcp_servers"]["r"].get("http_headers").is_some());
}

#[test]
fn an_sse_remote_is_proxied_because_codex_has_no_sse_form() {
    let spec = ServerSpec {
        name: "r".into(),
        transport: Transport::Remote(Remote {
            kind: RemoteKind::Sse,
            url: "https://x/sse".into(),
            headers: Box::new([]),
            auth: Auth::None,
        }),
        presence: Presence::Always,
        scope: RunnerScope::default(),
        timeout_secs: None,
    };
    let json = config([&spec], at());
    assert_eq!(json["mcp_servers"]["r"]["args"][0], "mcp-proxy");
}

#[test]
fn a_session_bound_env_pair_drops_before_the_thread_has_an_id() {
    let spec = ServerSpec::stdio(
        "agent-manager",
        "/opman",
        vec![Arg::lit("mcp-agent-manager"), Arg::Dir],
        vec![("OPENCODE_SESSION_ID".into(), Arg::SessionId)],
    );
    let json = config([&spec], Bind::new("/opman", "/proj", None));
    let entry = &json["mcp_servers"]["agent-manager"];
    assert_eq!(entry["args"][1], "/proj");
    assert!(entry.get("env").is_none());
}
