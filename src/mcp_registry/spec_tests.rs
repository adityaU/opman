//! Scoping and session-dependence, the two spec-level questions the renderers ask.

use super::*;

fn acp(id: &str) -> RunnerKind {
    RunnerKind::Acp(id.to_string())
}

#[test]
fn an_empty_scope_admits_every_runner() {
    let scope = RunnerScope::default();
    assert!(scope.admits(&RunnerKind::Codex));
    assert!(scope.admits(&acp("gemini")));
}

#[test]
fn an_allow_list_excludes_everything_else() {
    let scope = RunnerScope::new(vec![RunnerKind::Codex], Vec::new());
    assert!(scope.admits(&RunnerKind::Codex));
    assert!(!scope.admits(&RunnerKind::Opencode));
}

#[test]
fn a_deny_list_wins_over_an_allow_list() {
    let scope = RunnerScope::new(
        vec![RunnerKind::Codex, RunnerKind::Opencode],
        vec![RunnerKind::Opencode],
    );
    assert!(scope.admits(&RunnerKind::Codex));
    assert!(!scope.admits(&RunnerKind::Opencode));
}

#[test]
fn acp_agents_scope_by_their_config_id() {
    let scope = RunnerScope::new(vec![acp("claude")], Vec::new());
    assert!(scope.admits(&acp("claude")));
    assert!(!scope.admits(&acp("gemini")));
}

#[test]
fn binds_session_sees_a_session_placeholder_in_env() {
    let spec = ServerSpec::stdio(
        "x",
        "/opman",
        vec![Arg::lit("mcp")],
        vec![("OPENCODE_SESSION_ID".into(), Arg::SessionId)],
    );
    assert!(spec.binds_session());
}

#[test]
fn binds_session_sees_one_nested_in_mixed_text() {
    let spec = ServerSpec::stdio(
        "x",
        "/opman",
        vec![Arg::Mixed(Box::new([Arg::lit("--id="), Arg::SessionId]))],
        Vec::new(),
    );
    assert!(spec.binds_session());
}

#[test]
fn a_spec_with_no_session_placeholder_does_not_bind_one() {
    let spec = ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new());
    assert!(!spec.binds_session());
}

#[test]
fn only_a_credential_free_remote_skips_the_proxy() {
    assert!(!Auth::None.needs_proxy());
    assert!(Auth::StaticHeader.needs_proxy());
    assert!(Auth::Oauth.needs_proxy());
}

#[test]
fn presence_always_is_met_without_touching_the_environment() {
    assert!(Presence::Always.met());
}

#[test]
fn presence_env_follows_the_variable() {
    let name = "OPMAN_SPEC_TEST_PRESENCE_VAR";
    assert!(!Presence::Env(name.into()).met());
}
