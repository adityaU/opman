//! The MCP OAuth login endpoints.
//!
//! No test here talks to a real authorization server, so what is covered is every path
//! that rejects, reports, or cleans up — the ones a first real login is least likely to
//! exercise and most likely to need.
//!
//! `OPMAN_MCP_CONFIG` redirects `mcp.json` and `XDG_CONFIG_HOME` redirects the token
//! store, so nothing reads or writes the developer's real configuration or credentials.

use super::*;

use crate::web::test_support::{test_server_state, ConfigRedirect};

fn open() -> AuthUser {
    AuthUser {
        subject: String::new(),
    }
}

fn oauth_server(url: &str) -> ServerConfig {
    ServerConfig {
        r#type: "http".into(),
        url: url.into(),
        auth: "oauth".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn login_on_an_undeclared_server_is_404() {
    let _env = ConfigRedirect::new();
    let error = start_login(open(), State(test_server_state()), Path("nope".into()))
        .await
        .expect_err("undeclared server");
    assert!(matches!(error, WebError::NotFound(_)), "{error}");
}

#[tokio::test]
async fn login_on_a_server_that_does_not_use_oauth_is_rejected() {
    let env = ConfigRedirect::new();
    env.declare(
        "plain",
        ServerConfig {
            command: "npx".into(),
            ..Default::default()
        },
    );
    let error = start_login(open(), State(test_server_state()), Path("plain".into()))
        .await
        .expect_err("non-oauth server");
    assert!(matches!(error, WebError::BadRequest(_)), "{error}");
}

#[tokio::test]
async fn login_on_an_oauth_server_with_no_url_is_rejected() {
    let env = ConfigRedirect::new();
    env.declare(
        "urlless",
        ServerConfig {
            command: "npx".into(),
            auth: "oauth".into(),
            ..Default::default()
        },
    );
    let error = start_login(open(), State(test_server_state()), Path("urlless".into()))
        .await
        .expect_err("no url to authorize against");
    assert!(matches!(error, WebError::BadRequest(_)), "{error}");
}

#[tokio::test]
async fn login_reports_the_discovery_failure_rather_than_waiting_it_out() {
    let env = ConfigRedirect::new();
    // Port 1 refuses immediately, so this exercises the path where the flow dies before
    // producing an authorize URL — which without the race would sit out the full timeout.
    env.declare("dead", oauth_server("http://127.0.0.1:1/mcp"));
    let error = start_login(open(), State(test_server_state()), Path("dead".into()))
        .await
        .expect_err("discovery failure");
    assert!(matches!(error, WebError::BadRequest(_)), "{error}");
}

#[tokio::test]
async fn login_rejects_a_name_that_could_not_be_a_server() {
    let _env = ConfigRedirect::new();
    let error = start_login(open(), State(test_server_state()), Path("../etc".into()))
        .await
        .expect_err("bad name");
    assert!(matches!(error, WebError::BadRequest(_)), "{error}");
}

#[tokio::test]
async fn finishing_a_login_nobody_started_is_404() {
    let _env = ConfigRedirect::new();
    let error = finish_login(
        open(),
        State(test_server_state()),
        Path("linear".into()),
        Json(FinishLogin {
            url: "http://127.0.0.1:1/callback?code=a&state=b".into(),
        }),
    )
    .await
    .expect_err("no pending login");
    assert!(matches!(error, WebError::NotFound(_)), "{error}");
}

#[tokio::test]
async fn finishing_with_junk_is_rejected_before_anything_is_looked_up() {
    let _env = ConfigRedirect::new();
    let error = finish_login(
        open(),
        State(test_server_state()),
        Path("linear".into()),
        Json(FinishLogin {
            url: "i clicked cancel".into(),
        }),
    )
    .await
    .expect_err("unusable paste");
    assert!(matches!(error, WebError::BadRequest(_)), "{error}");
}

#[tokio::test]
async fn logout_is_idempotent_and_announces_itself() {
    let _env = ConfigRedirect::new();
    let state = test_server_state();
    let mut events = state.event_tx.subscribe();

    let response = logout_server(open(), State(state.clone()), Path("linear".into()))
        .await
        .expect("logout");
    assert_eq!(response.0["status"], "signed out");
    assert!(matches!(events.try_recv(), Ok(WebEvent::McpServersChanged)));

    // Again, with no credential to remove: still fine, because the button's promise is
    // "you are signed out", not "a file was deleted".
    let _again = logout_server(open(), State(state), Path("linear".into()))
        .await
        .expect("second logout");
}
