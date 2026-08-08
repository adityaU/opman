//! `mcp.json` parsing, overlay semantics, and the shape inference that decides whether
//! an entry is a child process or an endpoint.

use super::*;
use crate::mcp_registry::spec::{Auth, Transport};

fn parse(raw: &str) -> McpConfig {
    serde_json::from_str(raw).expect("valid test config")
}

#[test]
fn a_command_entry_infers_stdio() {
    let cfg = parse(r#"{"servers":{"a":{"command":"npx","args":["-y","x"]}}}"#);
    let spec = cfg.servers["a"].to_spec("a").expect("spec");
    let Transport::Stdio(stdio) = spec.transport() else {
        panic!("expected stdio");
    };
    assert_eq!(&*stdio.command, "npx");
    assert_eq!(stdio.args.len(), 2);
}

#[test]
fn a_url_entry_infers_streamable_http() {
    let cfg = parse(r#"{"servers":{"a":{"url":"https://x/mcp"}}}"#);
    let spec = cfg.servers["a"].to_spec("a").expect("spec");
    assert!(matches!(spec.transport(), Transport::Remote(_)));
}

#[test]
fn an_explicit_type_beats_inference() {
    let cfg = parse(r#"{"servers":{"a":{"type":"sse","url":"https://x/sse"}}}"#);
    let spec = cfg.servers["a"].to_spec("a").expect("spec");
    let Transport::Remote(remote) = spec.transport() else {
        panic!("expected remote");
    };
    assert_eq!(remote.kind, RemoteKind::Sse);
}

#[test]
fn headers_imply_a_credential_even_without_an_auth_field() {
    // Treating a header-bearing server as public would leak it into runner argv.
    let cfg = parse(r#"{"servers":{"a":{"url":"https://x/mcp","headers":{"X":"y"}}}}"#);
    let spec = cfg.servers["a"].to_spec("a").expect("spec");
    let Transport::Remote(remote) = spec.transport() else {
        panic!("expected remote");
    };
    assert_eq!(remote.auth, Auth::StaticHeader);
    assert!(remote.auth.needs_proxy());
}

#[test]
fn a_proxied_server_gets_a_default_ceiling() {
    let cfg = parse(r#"{"servers":{"a":{"url":"https://x/mcp","auth":"oauth"}}}"#);
    let spec = cfg.servers["a"].to_spec("a").expect("spec");
    assert_eq!(
        spec.timeout_secs(),
        Some(crate::mcp_registry::PROXY_TIMEOUT_SECS)
    );
}

#[test]
fn an_explicit_timeout_is_not_overridden() {
    let cfg = parse(r#"{"servers":{"a":{"url":"https://x/mcp","auth":"oauth","timeoutSecs":30}}}"#);
    assert_eq!(
        cfg.servers["a"].to_spec("a").expect("spec").timeout_secs(),
        Some(30)
    );
}

#[test]
fn a_public_stdio_server_gets_no_ceiling() {
    let cfg = parse(r#"{"servers":{"a":{"command":"npx"}}}"#);
    assert_eq!(
        cfg.servers["a"].to_spec("a").expect("spec").timeout_secs(),
        None
    );
}

#[test]
fn a_disabled_entry_yields_no_spec() {
    let cfg = parse(r#"{"servers":{"a":{"command":"npx","enabled":false}}}"#);
    assert!(cfg.servers["a"].to_spec("a").is_none());
}

#[test]
fn entries_are_enabled_by_default() {
    let cfg = parse(r#"{"servers":{"a":{"command":"npx"}}}"#);
    assert!(cfg.servers["a"].enabled);
}

#[test]
fn an_entry_with_neither_command_nor_url_is_ignored_not_fatal() {
    let cfg = parse(r#"{"servers":{"a":{"args":["x"]}}}"#);
    assert!(cfg.servers["a"].to_spec("a").is_none());
}

#[test]
fn an_entry_without_a_transport_is_a_patch_not_a_definition() {
    let cfg = parse(r#"{"servers":{"time":{"timeoutSecs":120}}}"#);
    assert!(!cfg.servers["time"].defines_transport());
    let cfg = parse(r#"{"servers":{"a":{"command":"npx"}}}"#);
    assert!(cfg.servers["a"].defines_transport());
    let cfg = parse(r#"{"servers":{"a":{"url":"https://x/mcp"}}}"#);
    assert!(cfg.servers["a"].defines_transport());
}

#[test]
fn a_patch_keeps_the_builtin_launch_command() {
    use crate::mcp_registry::spec::Arg;
    let builtin = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    let cfg = parse(r#"{"servers":{"time":{"timeoutSecs":120}}}"#);
    let patched = cfg.servers["time"].patch(builtin).expect("still enabled");
    assert_eq!(patched.timeout_secs(), Some(120));
    let Transport::Stdio(stdio) = patched.transport() else {
        panic!("expected stdio");
    };
    assert_eq!(stdio.args[0], Arg::lit("mcp-time"));
}

#[test]
fn a_patch_that_disables_removes_the_builtin() {
    use crate::mcp_registry::spec::Arg;
    let builtin = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    let cfg = parse(r#"{"servers":{"time":{"enabled":false}}}"#);
    assert!(cfg.servers["time"].patch(builtin).is_none());
}

#[test]
fn a_patch_can_narrow_which_runners_see_a_builtin() {
    use crate::mcp_registry::spec::Arg;
    use opman_backend_contracts::RunnerKind;
    let builtin = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    let cfg = parse(r#"{"servers":{"time":{"runners":["codex"]}}}"#);
    let patched = cfg.servers["time"].patch(builtin).expect("still enabled");
    assert!(patched.admits(&RunnerKind::Acp("codex".to_string())));
    assert!(!patched.admits(&RunnerKind::Opencode));
}

#[test]
fn unknown_fields_are_ignored() {
    let cfg = parse(r#"{"servers":{"a":{"command":"npx","futureField":123}}}"#);
    assert_eq!(cfg.servers["a"].command, "npx");
}

#[test]
fn a_round_trip_keeps_the_document_lean() {
    let cfg = parse(r#"{"servers":{"a":{"command":"npx"}}}"#);
    let out = serde_json::to_string(&cfg).expect("serialize");
    // Empty optionals are skipped, so hand-written files stay readable.
    assert!(!out.contains("\"headers\""));
    assert!(!out.contains("\"clientId\""));
    assert!(out.contains("\"command\":\"npx\""));
}
