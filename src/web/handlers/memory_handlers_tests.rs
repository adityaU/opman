//! Coverage tests for the personal-memory, autonomy, and active-memory handlers.
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use serde_json::json;

fn body_json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap_or(serde_json::Value::Null)
}

fn router() -> axum::Router {
    test_router(test_server_state())
}

// ── personal memory ─────────────────────────────────────────────────

async fn create_memory(state: &ServerState) -> String {
    let (status, body) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/memory",
        Some(json!({ "label": "l", "content": "c", "scope": "global" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body_json(&body)["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn memory_list_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/memory", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["memory"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn memory_create_update_delete() {
    let state = test_server_state();
    let id = create_memory(&state).await;

    let (s1, body) = send_json(
        test_router(state.clone()),
        "PATCH",
        &format!("/api/memory/{id}"),
        Some(json!({ "label": "updated" })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(body_json(&body)["label"], "updated");

    let (s2, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        &format!("/api/memory/{id}"),
        None,
    )
    .await;
    assert_eq!(s2, StatusCode::OK);

    let (s3, _) = send_json(
        test_router(state),
        "DELETE",
        &format!("/api/memory/{id}"),
        None,
    )
    .await;
    assert_eq!(s3, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn memory_update_not_found() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/memory/missing",
        Some(json!({ "label": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── autonomy ────────────────────────────────────────────────────────

#[tokio::test]
async fn autonomy_get_and_update() {
    let state = test_server_state();
    let (s1, _) = send_json(test_router(state.clone()), "GET", "/api/autonomy", None).await;
    assert_eq!(s1, StatusCode::OK);

    let (s2, body) = send_json(
        test_router(state),
        "POST",
        "/api/autonomy",
        Some(json!({ "mode": "observe" })),
    )
    .await;
    assert_eq!(s2, StatusCode::OK);
    assert_eq!(body_json(&body)["mode"], "observe");
}

// ── /api/memory/active ──────────────────────────────────────────────

#[tokio::test]
async fn active_memory_no_params_ok() {
    let (status, body) = send_json(router(), "GET", "/api/memory/active", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("memory").is_some());
}

#[tokio::test]
async fn active_memory_with_params_ok() {
    let (status, _) = send_json(
        router(),
        "GET",
        "/api/memory/active?project_index=0&session_id=s1",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
