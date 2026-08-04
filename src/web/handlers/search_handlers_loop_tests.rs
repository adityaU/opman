//! Generated coverage tests (wave 2) for `search_handlers.rs`.
//!
//! Drives the `search_messages` fetch loop with a *seeded* session so the
//! loop body executes (the upstream opencode call to the dead `base_url()`
//! errors → the `Err(_) => continue` arm runs). The earlier file-tests only
//! exercised the empty-session path, which skips the loop entirely.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

fn sess(id: &str, dir: &str) -> crate::app::SessionInfo {
    crate::app::SessionInfo {
        id: id.into(),
        title: format!("title-{id}"),
        parent_id: String::new(),
        directory: dir.into(),
        time: crate::app::SessionTime {
            created: 1,
            updated: 2,
        },
    }
}

async fn body_json<T: IntoResponse>(
    r: Result<T, WebError>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let resp = r.into_response();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

#[tokio::test]
async fn search_messages_with_sessions_upstream_fails_continues() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    // Seed two sessions so the loop iterates; every upstream fetch to the dead
    // base_url errors → `Err(_) => continue`, yielding zero matches.
    state
        .web_state
        .add_and_activate_session(0, sess("s1", tmp.path().to_str().unwrap()))
        .await;
    state
        .web_state
        .add_and_activate_session(0, sess("s2", tmp.path().to_str().unwrap()))
        .await;

    let (st, v) = body_json(
        search_messages(
            State(state),
            auth(),
            Path(0usize),
            Query(SearchQuery {
                q: "anything".into(),
                limit: 50,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["total"], 0);
    assert_eq!(v["query"], "anything");
}

#[tokio::test]
async fn search_messages_limit_is_capped_at_200() {
    // limit=9999 exercises the `params.limit.min(200)` cap line with a session
    // present so the loop is entered before the fetch fails.
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    state
        .web_state
        .add_and_activate_session(0, sess("s1", tmp.path().to_str().unwrap()))
        .await;

    let (st, v) = body_json(
        search_messages(
            State(state),
            auth(),
            Path(0usize),
            Query(SearchQuery {
                q: "term".into(),
                limit: 9999,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["total"], 0);
}
