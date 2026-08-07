//! The tools endpoint. The probe itself is covered in `mcp_probe`; what matters here is
//! that a bad name is refused with a sentence, a probe outcome is reported rather than
//! thrown away as a status code, and the one thing the endpoint genuinely cannot do
//! without says so.

use super::*;

use std::path::PathBuf;

use crate::web::test_support::{test_server_state, ConfigRedirect};
use crate::web::web_state::WebStateHandle;

fn open() -> AuthUser {
    AuthUser {
        subject: String::new(),
    }
}

/// A server launches in the active project's directory, so the probe needs one.
fn state_in(dir: &str) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("probe".to_string(), PathBuf::from(dir))]);
    state
}

#[test]
fn a_name_that_could_not_be_a_server_is_refused_with_a_reason() {
    let error = validate_name("../../etc/passwd").expect_err("path segments are not names");

    match error {
        WebError::BadRequest(message) => assert!(message.contains("1–64"), "{message}"),
        other => panic!("expected a bad request, got {other}"),
    }
}

#[test]
fn a_legal_name_is_trimmed_and_kept() {
    assert_eq!(
        validate_name("  agent-manager ").expect("legal"),
        "agent-manager"
    );
}

/// The endpoint answers 200 with the outcome even when the server cannot be reached, so
/// the page can say *which* server is broken instead of showing a blank panel.
#[tokio::test]
async fn an_undeclared_server_answers_with_an_unavailable_outcome() {
    let _redirect = ConfigRedirect::new();
    let state = state_in("/tmp");

    let Json(outcome) = list_tools(open(), State(state), Path("not-declared".to_string()))
        .await
        .expect("the probe outcome is the payload, not an error");

    assert!(
        matches!(outcome, Catalog::Unavailable { .. }),
        "{outcome:?}"
    );
}

/// The one genuine precondition. A probe has to launch the server somewhere, and guessing
/// a directory would run it against the wrong project's files.
#[tokio::test]
async fn without_an_active_project_the_request_says_so() {
    let _redirect = ConfigRedirect::new();

    let error = list_tools(open(), State(test_server_state()), Path("time".to_string()))
        .await
        .err()
        .expect("there is nowhere to launch it");

    assert!(matches!(error, WebError::BadRequest(_)), "{error}");
}
