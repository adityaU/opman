//! Regression coverage for directory-browser symbolic links.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::{BrowseDirsRequest, ServerState};
use axum::extract::{Json, State};
use axum::response::IntoResponse;

fn auth() -> AuthUser {
    AuthUser {
        subject: "test".into(),
    }
}

async fn body(result: WebResult<impl IntoResponse>) -> (axum::http::StatusCode, serde_json::Value) {
    let response = result.into_response();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[cfg(unix)]
#[tokio::test]
async fn browse_lists_directory_symlink_but_skips_broken_symlink() {
    let state: ServerState = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("real")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("real"), tmp.path().join("linked")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("missing"), tmp.path().join("broken")).unwrap();

    let (status, value) = body(
        browse_dirs(
            State(state),
            auth(),
            Json(BrowseDirsRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let entries = value["entries"].as_array().unwrap();
    let linked = entries.iter().find(|entry| entry["name"] == "linked");
    assert_eq!(
        linked.map(|entry| entry["is_symlink"].as_bool()),
        Some(Some(true))
    );
    assert!(!entries.iter().any(|entry| entry["name"] == "broken"));
}
