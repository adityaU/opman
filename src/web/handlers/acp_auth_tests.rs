use crate::acp_engine::diagnose::{Hello, Offer, Probe, Session};

use super::*;

fn probe(session: Session, methods: &[&str]) -> Probe {
    Probe {
        hello: Hello {
            protocol: 1,
            name: None,
            version: None,
            auth_methods: methods.iter().map(|m| m.to_string()).collect(),
            loads_sessions: false,
        },
        session,
    }
}

#[test]
fn a_session_means_connected_and_no_login_is_offered() {
    let view = view_of("a", "/bin/opman", Ok(probe(Session::Offered(Offer::default()), &["Google"])));
    assert_eq!(view.state, AuthState::Connected);
    assert_eq!(view.login_command, None);
    assert_eq!(view.methods, ["Google"]);
}

#[test]
fn login_required_offers_the_login_command() {
    let view = view_of("antigravity", "/opt/my opman/opman", Ok(probe(Session::LoginRequired, &["Google"])));
    assert_eq!(view.state, AuthState::NotConnected);
    assert_eq!(
        view.login_command.as_deref(),
        Some("'/opt/my opman/opman' acp-login antigravity")
    );
}

#[test]
fn an_agent_that_lists_no_methods_has_nothing_to_log_in_with() {
    let view = view_of("a", "opman", Ok(probe(Session::LoginRequired, &[])));
    assert_eq!(view.state, AuthState::NotConnected);
    assert_eq!(view.login_command, None);
}

#[test]
fn a_failed_session_keeps_the_agents_reason() {
    let view = view_of("a", "opman", Ok(probe(Session::Failed("quota".into()), &["Key"])));
    assert_eq!(view.detail, "quota");
    assert!(view.login_command.is_some());
}

#[test]
fn an_agent_that_cannot_start_is_not_connected_with_its_error() {
    let view = view_of("a", "opman", Err("not found on PATH".into()));
    assert_eq!(view.state, AuthState::NotConnected);
    assert_eq!(view.detail, "not found on PATH");
    assert_eq!(view.login_command, None);
}

#[test]
fn quoting_survives_a_single_quote() {
    assert_eq!(quoted("it's"), r"'it'\''s'");
}
