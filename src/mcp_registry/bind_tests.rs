//! Binding, and the proxy-substitution matrix that is the point of the whole module.

use super::*;
use crate::mcp_registry::spec::{Auth, Presence, Remote, RemoteKind, RunnerScope};

fn remote_spec(kind: RemoteKind, auth: Auth) -> ServerSpec {
    ServerSpec {
        name: "linear".into(),
        transport: Transport::Remote(Remote {
            kind,
            url: "https://mcp.linear.app/sse".into(),
            headers: Box::new([]),
            auth,
        }),
        presence: Presence::Always,
        scope: RunnerScope::default(),
        timeout_secs: None,
    }
}

fn at<'a>() -> Bind<'a> {
    Bind::new("/opman", "/proj", Some("ses1"))
}

// -- the 2x2: (auth needs a credential) x (runner can dial this transport) ------------

#[test]
fn a_public_remote_a_runner_can_dial_stays_remote() {
    let spec = remote_spec(RemoteKind::Http, Auth::None);
    let Some(Wire::Remote(remote)) = spec.bind(at(), RemoteCaps::HTTP_ONLY) else {
        panic!("expected a remote wire");
    };
    assert_eq!(remote.url, "https://mcp.linear.app/sse");
}

#[test]
fn a_public_remote_a_runner_cannot_dial_is_proxied_rather_than_dropped() {
    let spec = remote_spec(RemoteKind::Sse, Auth::None);
    let Some(Wire::Stdio(stdio)) = spec.bind(at(), RemoteCaps::HTTP_ONLY) else {
        panic!("expected the proxy stdio wire");
    };
    assert_eq!(stdio.command, "/opman");
    assert_eq!(stdio.args, vec!["mcp-proxy", "linear"]);
}

#[test]
fn a_credential_bearing_remote_is_proxied_even_when_the_runner_could_dial_it() {
    // The whole point: the token must never reach the runner, however capable it is.
    for auth in [Auth::StaticHeader, Auth::Oauth] {
        let spec = remote_spec(RemoteKind::Http, auth);
        let Some(Wire::Stdio(stdio)) = spec.bind(at(), RemoteCaps::CLAUDE) else {
            panic!("expected the proxy stdio wire for {auth:?}");
        };
        assert_eq!(stdio.args, vec!["mcp-proxy", "linear"]);
    }
}

#[test]
fn a_stdio_only_runner_proxies_every_remote() {
    let spec = remote_spec(RemoteKind::Http, Auth::None);
    assert!(matches!(
        spec.bind(at(), RemoteCaps::STDIO_ONLY),
        Some(Wire::Stdio(_))
    ));
}

// -- placeholder resolution ----------------------------------------------------------

#[test]
fn an_unresolvable_session_in_an_env_value_drops_only_that_pair() {
    // Exactly how the agent-manager bridge already behaves on Codex's thread/start.
    let spec = ServerSpec::stdio(
        "agent-manager",
        "/opman",
        vec![Arg::lit("mcp-agent-manager"), Arg::Dir],
        vec![("OPENCODE_SESSION_ID".into(), Arg::SessionId)],
    );
    let bind = Bind::new("/opman", "/proj", None);
    let Some(Wire::Stdio(stdio)) = spec.bind(bind, RemoteCaps::STDIO_ONLY) else {
        panic!("server should survive");
    };
    assert!(stdio.env.is_empty());
    assert_eq!(stdio.args, vec!["mcp-agent-manager", "/proj"]);
}

#[test]
fn an_unresolvable_session_in_a_positional_arg_drops_the_whole_server() {
    // A positional hole cannot be skipped without changing what the command means.
    let spec = ServerSpec::stdio("x", "/opman", vec![Arg::SessionId], Vec::new());
    let bind = Bind::new("/opman", "/proj", None);
    assert!(spec.bind(bind, RemoteCaps::STDIO_ONLY).is_none());
}

#[test]
fn dir_resolves_to_the_bound_directory() {
    let spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp"), Arg::Dir], Vec::new());
    let Some(Wire::Stdio(stdio)) = spec.bind(at(), RemoteCaps::STDIO_ONLY) else {
        panic!("expected stdio");
    };
    assert_eq!(stdio.args, vec!["mcp", "/proj"]);
}

#[test]
fn mixed_text_is_concatenated() {
    let spec = ServerSpec::stdio(
        "x",
        "/opman",
        vec![Arg::Mixed(Box::new([Arg::lit("--dir="), Arg::Dir]))],
        Vec::new(),
    );
    let Some(Wire::Stdio(stdio)) = spec.bind(at(), RemoteCaps::STDIO_ONLY) else {
        panic!("expected stdio");
    };
    assert_eq!(stdio.args, vec!["--dir=/proj"]);
}

#[test]
fn an_unset_env_placeholder_drops_its_pair() {
    let spec = ServerSpec::stdio(
        "x",
        "/opman",
        vec![Arg::lit("mcp")],
        vec![("K".into(), Arg::Env("OPMAN_BIND_TEST_UNSET".into()))],
    );
    let Some(Wire::Stdio(stdio)) = spec.bind(at(), RemoteCaps::STDIO_ONLY) else {
        panic!("expected stdio");
    };
    assert!(stdio.env.is_empty());
}

// -- presence ------------------------------------------------------------------------

#[test]
fn an_unmet_presence_condition_withholds_the_server() {
    let spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp")], Vec::new())
        .with_presence(Presence::Env("OPMAN_BIND_TEST_ABSENT".into()));
    assert!(spec.bind(at(), RemoteCaps::STDIO_ONLY).is_none());
}

#[test]
fn stdio_transport_ignores_remote_capabilities() {
    let spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp")], Vec::new());
    assert!(spec.bind(at(), RemoteCaps::STDIO_ONLY).is_some());
    assert!(spec.bind(at(), RemoteCaps::CLAUDE).is_some());
}
