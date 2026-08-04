//! Generated tests for `dashboard_ext_handlers.rs`.
//!
//! Routines, delegation, and workspace snapshots are all backed by the
//! in-memory web state, so we assert real success/not-found behaviour.

use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use serde_json::json;

fn body_json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

async fn create_routine(state: &ServerState) -> String {
    let (status, body) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/routines",
        Some(json!({ "name": "r", "trigger": "manual", "action": "send_message" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body_json(&body)["id"].as_str().unwrap().to_string()
}

// ── routines ────────────────────────────────────────────────────────

#[tokio::test]
async fn routines_list_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/routines", None).await;
    assert_eq!(status, StatusCode::OK);
    let v = body_json(&body);
    assert_eq!(v["routines"].as_array().unwrap().len(), 0);
    assert_eq!(v["runs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn routine_create() {
    let state = test_server_state();
    let id = create_routine(&state).await;
    assert!(id.starts_with("routine-") || !id.is_empty());
    let (status, body) = send_json(test_router(state), "GET", "/api/routines", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["routines"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn routine_update_ok_and_not_found() {
    let state = test_server_state();
    let id = create_routine(&state).await;
    let (s1, body) = send_json(
        test_router(state.clone()),
        "PATCH",
        &format!("/api/routines/{id}"),
        Some(json!({ "name": "renamed", "enabled": false })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(body_json(&body)["name"], "renamed");

    let (s2, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/routines/missing",
        Some(json!({ "name": "x" })),
    )
    .await;
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn routine_delete_ok_and_not_found() {
    let state = test_server_state();
    let id = create_routine(&state).await;
    let (s1, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        &format!("/api/routines/{id}"),
        None,
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    let (s2, _) = send_json(test_router(state), "DELETE", "/api/routines/missing", None).await;
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn routine_run_with_summary_records_run() {
    let state = test_server_state();
    let id = create_routine(&state).await;
    let (status, body) = send_json(
        test_router(state),
        "POST",
        &format!("/api/routines/{id}/run"),
        Some(json!({ "summary": "did the thing" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["summary"], "did the thing");
}

#[tokio::test]
async fn routine_run_without_prompt_fails() {
    // A send_message routine with no prompt → execute_routine returns Err → 400.
    let state = test_server_state();
    let id = create_routine(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        &format!("/api/routines/{id}/run"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── delegation ──────────────────────────────────────────────────────

async fn create_delegated(state: &ServerState) -> String {
    let (status, body) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/delegation",
        Some(json!({ "title": "t", "assignee": "a", "scope": "s" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body_json(&body)["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn delegation_list_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/delegation", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn delegation_create_update_delete() {
    let state = test_server_state();
    let id = create_delegated(&state).await;

    let (s1, body) = send_json(
        test_router(state.clone()),
        "PATCH",
        &format!("/api/delegation/{id}"),
        Some(json!({ "status": "running" })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(body_json(&body)["status"], "running");

    let (s2, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        &format!("/api/delegation/{id}"),
        None,
    )
    .await;
    assert_eq!(s2, StatusCode::OK);

    let (s3, _) = send_json(
        test_router(state),
        "DELETE",
        &format!("/api/delegation/{id}"),
        None,
    )
    .await;
    assert_eq!(s3, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delegation_update_not_found() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/delegation/missing",
        Some(json!({ "status": "completed" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── workspaces ──────────────────────────────────────────────────────

fn snapshot(name: &str) -> serde_json::Value {
    json!({
        "snapshot": {
            "name": name,
            "created_at": "2020-01-01T00:00:00Z",
            "panels": { "sidebar": true, "terminal": true, "editor": false, "git": true }
        }
    })
}

#[tokio::test]
async fn workspaces_list_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/workspaces", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["workspaces"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn workspace_save_list_and_delete() {
    let state = test_server_state();
    let (s1, _) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/workspaces",
        Some(snapshot("w1")),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    let (s2, body) = send_json(test_router(state.clone()), "GET", "/api/workspaces", None).await;
    assert_eq!(s2, StatusCode::OK);
    let ws = body_json(&body);
    assert_eq!(ws["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(ws["workspaces"][0]["name"], "w1");

    let (s3, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        "/api/workspaces?name=w1",
        None,
    )
    .await;
    assert_eq!(s3, StatusCode::OK);

    let (s4, _) = send_json(
        test_router(state),
        "DELETE",
        "/api/workspaces?name=w1",
        None,
    )
    .await;
    assert_eq!(s4, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workspace_delete_missing_not_found() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "DELETE",
        "/api/workspaces?name=ghost",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
