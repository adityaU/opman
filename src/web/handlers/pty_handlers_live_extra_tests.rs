//! Additional live-manager coverage: the opencode "no explicit session id" arm
//! driven through a *running* manager (existing live tests only pass an explicit
//! session id), and write/resize against a running manager with an unknown id
//! (the real-manager lookup-miss path, distinct from the no-op handle tests).

use super::*;

use crate::web::auth::AuthUser;
use crate::web::pty_manager::start_web_pty_manager;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use axum::extract::State;
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

fn live_state(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s.pty_mgr = start_web_pty_manager();
    s
}

async fn status<T: IntoResponse>(r: Result<T, WebError>) -> axum::http::StatusCode {
    r.into_response().status()
}

#[tokio::test]
async fn spawn_opencode_no_session_id_resolves_none_arm() {
    // No explicit session_id and no active session → the `None => active_session_id().await`
    // arm runs (yielding None) before spawn_opencode is invoked through the live manager.
    let tmp = tempfile::TempDir::new().unwrap();
    let state = live_state(tmp.path());
    let req = SpawnPtyRequest {
        kind: "opencode".into(),
        id: "oc-nosess".into(),
        rows: Some(24),
        cols: Some(80),
        session_id: None,
    };
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    // 500 when the opencode binary is absent (typical CI), 200 if installed.
    assert!(
        st == axum::http::StatusCode::INTERNAL_SERVER_ERROR || st == axum::http::StatusCode::OK,
        "unexpected status {st}"
    );
}

#[tokio::test]
async fn live_manager_write_resize_kill_unknown_id_not_found() {
    // A running manager with no such PTY id → write/resize/kill all report false → 400.
    let tmp = tempfile::TempDir::new().unwrap();
    let state = live_state(tmp.path());

    let data = BASE64.encode(b"x");
    let st = status(
        pty_write(
            State(state.clone()),
            auth(),
            axum::Json(PtyWriteRequest {
                id: "ghost".into(),
                data,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let st = status(
        pty_resize(
            State(state.clone()),
            auth(),
            axum::Json(PtyResizeRequest {
                id: "ghost".into(),
                rows: 30,
                cols: 100,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let st = status(
        pty_kill(
            State(state.clone()),
            auth(),
            axum::Json(PtyKillRequest { id: "ghost".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}
