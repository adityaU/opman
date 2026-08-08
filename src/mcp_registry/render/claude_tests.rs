//! Claude's `--mcp-config` shape.

use super::*;
use crate::mcp_registry::spec::{Arg, Auth, Presence, Remote, RunnerScope, Transport};

fn at<'a>() -> Bind<'a> {
    Bind::new("/opman", "/proj", Some("ses1"))
}

fn parsed(specs: &[ServerSpec]) -> serde_json::Value {
    let raw = config(specs.iter(), at()).expect("payload");
    serde_json::from_str(&raw).expect("valid json")
}

fn remote(auth: Auth, kind: RemoteKind) -> ServerSpec {
    ServerSpec {
        name: "r".into(),
        transport: Transport::Remote(Remote {
            kind,
            url: "https://x/mcp".into(),
            headers: Box::new([]),
            auth,
        }),
        presence: Presence::Always,
        scope: RunnerScope::default(),
        timeout_secs: None,
    }
}

#[test]
fn an_empty_set_yields_no_payload_so_the_flag_is_omitted() {
    assert!(config(std::iter::empty(), at()).is_none());
}

#[test]
fn stdio_uses_command_args_env() {
    let spec = ServerSpec::stdio(
        "terminal",
        "/opman",
        vec![Arg::lit("mcp"), Arg::Dir],
        vec![("OPENCODE_SESSION_ID".into(), Arg::SessionId)],
    );
    let json = parsed(&[spec]);
    let entry = &json["mcpServers"]["terminal"];
    assert_eq!(entry["command"], "/opman");
    assert_eq!(entry["args"][0], "mcp");
    assert_eq!(entry["args"][1], "/proj");
    assert_eq!(entry["env"]["OPENCODE_SESSION_ID"], "ses1");
}

#[test]
fn claude_can_dial_both_remote_flavours_itself() {
    for (kind, expected) in [(RemoteKind::Http, "http"), (RemoteKind::Sse, "sse")] {
        let json = parsed(&[remote(Auth::None, kind)]);
        assert_eq!(json["mcpServers"]["r"]["type"], expected);
        assert_eq!(json["mcpServers"]["r"]["url"], "https://x/mcp");
    }
}

#[test]
fn a_credential_bearing_remote_becomes_the_local_proxy() {
    let json = parsed(&[remote(Auth::Oauth, RemoteKind::Http)]);
    let entry = &json["mcpServers"]["r"];
    assert_eq!(entry["command"], "/opman");
    assert_eq!(entry["args"][0], "mcp-proxy");
    assert_eq!(entry["args"][1], "r");
    assert!(
        entry.get("url").is_none(),
        "the endpoint must not reach the runner"
    );
}

#[test]
fn the_timeout_is_emitted_in_milliseconds() {
    let mut spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp")], Vec::new());
    spec.timeout_secs = Some(900);
    assert_eq!(parsed(&[spec])["mcpServers"]["x"]["timeout"], 900_000);
}

#[test]
fn no_timeout_leaves_the_runner_default_alone() {
    let spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp")], Vec::new());
    assert!(parsed(&[spec])["mcpServers"]["x"].get("timeout").is_none());
}
