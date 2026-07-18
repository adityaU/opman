//! Extra branch coverage for `spawn_pty`'s `claude-attach` kind: the arm that
//! resolves the session id from web state (`active_session_id().await` Some-branch)
//! when the request omits `session_id`, then fails because the resolved session has
//! no running claude background agent (`short_id_for_session` → None → BadRequest).
//!
//! The `claude-attach` *success* path is genuinely unreachable in a unit test: it
//! requires the process-global claude `ENGINE` (a `OnceLock`) to hold a session
//! mapped to a live background short id, which only the embedded engine populates.

use super::*;

use axum::extract::State;
use axum::response::IntoResponse;
use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

fn auth() -> AuthUser {
    AuthUser { subject: "t".into() }
}

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

async fn status<T: IntoResponse>(r: Result<T, WebError>) -> axum::http::StatusCode {
    r.into_response().status()
}

#[tokio::test]
async fn claude_attach_resolves_active_session_but_no_agent_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    // Seed + activate a session so `active_session_id().await` returns Some — the
    // request omits session_id, so this drives the None → resolve-from-state arm.
    state
        .web_state
        .add_and_activate_session(
            0,
            crate::app::SessionInfo { id: "ses_active".into(), ..Default::default() },
        )
        .await;

    let req = SpawnPtyRequest {
        kind: "claude-attach".into(),
        id: "att-1".into(),
        rows: Some(24),
        cols: Some(80),
        session_id: None,
    };
    // Active session resolved, but it has no running claude agent → 400.
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}
