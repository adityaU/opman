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

fn spawn_req(kind: PtyKind, id: &str) -> SpawnPtyRequest {
    SpawnPtyRequest {
        kind,
        id: id.into(),
        rows: Some(24),
        cols: Some(80),
        project: None,
        label: None,
        session_id: None,
    }
}

/// The sessions endpoint, decoded.
async fn sessions(state: &ServerState) -> Vec<serde_json::Value> {
    let resp = pty_sessions(State(state.clone()), auth())
        .await
        .expect("sessions always answers")
        .into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("a small JSON body");
    serde_json::from_slice(&bytes).expect("an array of sessions")
}

#[tokio::test]
async fn spawn_shell_success_then_write_resize_list_kill() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = live_state(tmp.path());

    // Spawn a real shell PTY → success path returns 200 with { id, ok: true }.
    let resp = spawn_pty(
        State(state.clone()),
        auth(),
        axum::Json(spawn_req(PtyKind::Shell, "live-1")),
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

    // sessions → the shell is there, tagged with its project and a label.
    let listed = sessions(&state).await;
    let entry = listed
        .iter()
        .find(|s| s["id"] == "live-1")
        .expect("the spawned shell is listed");
    assert_eq!(entry["kind"], "shell");
    assert_eq!(entry["project"], tmp.path().to_string_lossy().as_ref());
    assert_eq!(entry["label"], "Shell 1", "the manager numbers it");

    // rename → the picker's label changes, the PTY does not.
    let st = status(
        pty_rename(
            State(state.clone()),
            auth(),
            axum::Json(PtyRenameRequest {
                id: "live-1".into(),
                label: "  Build  ".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let listed = sessions(&state).await;
    let entry = listed.iter().find(|s| s["id"] == "live-1").expect("still there");
    assert_eq!(entry["label"], "Build", "trimmed");

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
    let mut req = spawn_req(PtyKind::Opencode, "oc-1");
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
async fn sessions_report_a_live_shell_activity() {
    let dir = tempfile::tempdir().unwrap();
    let state = live_state(dir.path());
    let ok = spawn_pty(
        State(state.clone()),
        auth(),
        axum::Json(spawn_req(PtyKind::Shell, "act1")),
    )
    .await
    .is_ok();
    assert!(ok, "shell spawns");

    let listed = sessions(&state).await;
    let entry = listed
        .iter()
        .find(|s| s["id"] == "act1")
        .expect("the live PTY is listed");
    assert!(
        matches!(entry["activity"].as_str(), Some("idle") | Some("running")),
        "a live PTY reports one of the two states: {entry:?}"
    );
}

/// Two shells in one project are numbered; a third project starts again at 1.
#[tokio::test]
async fn labels_are_numbered_per_project() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let state = live_state(first.path());

    for id in ["a", "b"] {
        let req = spawn_req(PtyKind::Shell, id);
        spawn_pty(State(state.clone()), auth(), axum::Json(req))
            .await
            .expect("shell spawns");
    }
    let mut third = spawn_req(PtyKind::Shell, "c");
    third.project = Some(second.path().to_string_lossy().into_owned());
    spawn_pty(State(state.clone()), auth(), axum::Json(third))
        .await
        .expect("shell spawns");

    let listed = sessions(&state).await;
    let label = |id: &str| {
        listed
            .iter()
            .find(|s| s["id"] == id)
            .map(|s| s["label"].as_str().unwrap_or_default().to_owned())
            .unwrap_or_default()
    };
    let mut first_two = [label("a"), label("b")];
    first_two.sort();
    assert_eq!(first_two, ["Shell 1", "Shell 2"]);
    assert_eq!(label("c"), "Shell 1", "a new project counts from one");
}

/// Spawning onto an id that is already live must not start a second program on
/// top of the first — that would strand the running one with no reader.
#[tokio::test]
async fn spawning_a_live_id_twice_keeps_one_pty() {
    let dir = tempfile::tempdir().unwrap();
    let state = live_state(dir.path());
    for _ in 0..2 {
        spawn_pty(
            State(state.clone()),
            auth(),
            axum::Json(spawn_req(PtyKind::Shell, "same")),
        )
        .await
        .expect("spawn is safe to retry");
    }
    let listed = sessions(&state).await;
    assert_eq!(listed.iter().filter(|s| s["id"] == "same").count(), 1);
}
