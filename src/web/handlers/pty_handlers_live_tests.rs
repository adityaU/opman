//! Coverage tests that drive a *real* web PTY manager so the success branches
//! of spawn/write/resize/kill/list execute (Wave-1 only covered the manager-down
//! error paths against the no-op handle).

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

/// ServerState with a real (running) PTY manager and an active project dir.
fn live_state(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s.pty_mgr = start_web_pty_manager();
    s
}

async fn status<T: IntoResponse>(r: Result<T, WebError>) -> axum::http::StatusCode {
    r.into_response().status()
}

fn spawn_req(kind: &str, id: &str) -> SpawnPtyRequest {
    SpawnPtyRequest {
        kind: kind.into(),
        id: id.into(),
        rows: Some(24),
        cols: Some(80),
        session_id: None,
    }
}

#[tokio::test]
async fn spawn_shell_success_then_write_resize_list_kill() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = live_state(tmp.path());

    // Spawn a real shell PTY → success path returns 200 with { id, ok: true }.
    let resp = spawn_pty(
        State(state.clone()),
        auth(),
        axum::Json(spawn_req("shell", "live-1")),
    )
    .await
    .unwrap()
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["id"], "live-1");
    assert_eq!(v["ok"], true);

    // list → contains the id.
    let resp = pty_list(State(state.clone()), auth())
        .await
        .unwrap()
        .into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let ids: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert!(ids.contains(&"live-1".to_string()));

    // write valid base64 to the live PTY → OK (found).
    let data = BASE64.encode(b"echo hi\n");
    let st = status(
        pty_write(
            State(state.clone()),
            auth(),
            axum::Json(PtyWriteRequest {
                id: "live-1".into(),
                data,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // resize the live PTY → OK (found). Also exercises clamping.
    let st = status(
        pty_resize(
            State(state.clone()),
            auth(),
            axum::Json(PtyResizeRequest {
                id: "live-1".into(),
                rows: 9999,
                cols: 0,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // kill the live PTY → OK (found).
    let st = status(
        pty_kill(
            State(state.clone()),
            auth(),
            axum::Json(PtyKillRequest {
                id: "live-1".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // killing again → not found → 400.
    let st = status(
        pty_kill(
            State(state.clone()),
            auth(),
            axum::Json(PtyKillRequest {
                id: "live-1".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_opencode_with_explicit_session_id() {
    // Exercises the `Some(sid)` session-resolution branch for the opencode kind.
    // The opencode binary is typically absent, so the real manager returns Err
    // → 500 — but the branch under test has already run.
    let tmp = tempfile::TempDir::new().unwrap();
    let state = live_state(tmp.path());
    let mut req = spawn_req("opencode", "oc-1");
    req.session_id = Some("ses_explicit".into());
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    // The Some(sid) resolution branch has run regardless of the spawn outcome:
    // 500 if the opencode binary is absent, 200 if it is installed on this host.
    assert!(
        st == axum::http::StatusCode::INTERNAL_SERVER_ERROR || st == axum::http::StatusCode::OK,
        "unexpected status {st}"
    );
}

#[tokio::test]
async fn pty_activity_reports_a_live_shell() {
    let dir = tempfile::tempdir().unwrap();
    let state = live_state(dir.path());
    let ok = spawn_pty(State(state.clone()), auth(), axum::Json(spawn_req("shell", "act1")))
        .await
        .is_ok();
    assert!(ok, "shell spawns");

    let resp = pty_activity(State(state), auth())
        .await
        .unwrap()
        .into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let entry = v.get("act1").and_then(|s| s.as_str());
    assert!(
        matches!(entry, Some("idle") | Some("running")),
        "a live PTY reports one of the two states: {v:?}"
    );
}
