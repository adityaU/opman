//! PTY lifecycle and session-listing tests.

use super::*;

#[tokio::test]
async fn pty_kill_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_kill(
            State(state),
            auth(),
            axum::Json(PtyKillRequest { id: "x".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pty_rename_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_rename(
            State(state),
            auth(),
            axum::Json(PtyRenameRequest {
                id: "x".into(),
                label: "Build".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

/// A blank label would leave an unclickable row in the picker, so it is refused
/// before the manager is asked.
#[tokio::test]
async fn pty_rename_blank_label_400() {
    let state = test_server_state();
    let st = status(
        pty_rename(
            State(state),
            auth(),
            axum::Json(PtyRenameRequest {
                id: "x".into(),
                label: "   ".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pty_sessions_empty_ok() {
    let state = test_server_state();
    let resp = pty_sessions(State(state), auth())
        .await
        .unwrap()
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_array().expect("an array of sessions").len(), 0);
}
