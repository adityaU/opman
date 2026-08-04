//! Generated tests for `dashboard_handlers.rs`.
//!
//! Every endpoint here is backed by the in-memory web state (no upstream), so
//! we assert real success/not-found behaviour end-to-end through the router.

use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use serde_json::json;

fn body_json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

async fn create_mission(state: &ServerState) -> String {
    let (status, body) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/missions",
        Some(json!({ "goal": "achieve x" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body_json(&body)["id"].as_str().unwrap().to_string()
}

// ── overview / tree ─────────────────────────────────────────────────

#[tokio::test]
async fn sessions_overview_ok() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/sessions/overview", None).await;
    assert_eq!(status, StatusCode::OK);
    let v = body_json(&body);
    assert_eq!(v["total"], 0);
    assert_eq!(v["busy_count"], 0);
    assert!(v["sessions"].is_array());
}

#[tokio::test]
async fn sessions_tree_ok() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/sessions/tree", None).await;
    assert_eq!(status, StatusCode::OK);
    let v = body_json(&body);
    assert_eq!(v["total"], 0);
    assert!(v["roots"].is_array());
}

// ── presence ────────────────────────────────────────────────────────

#[tokio::test]
async fn presence_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/presence", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["clients"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn presence_register_then_list_then_deregister() {
    let state = test_server_state();
    let (s1, _) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/presence",
        Some(json!({ "client_id": "c1", "interface_type": "web", "focused_session": "sess" })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    let (s2, body) = send_json(test_router(state.clone()), "GET", "/api/presence", None).await;
    assert_eq!(s2, StatusCode::OK);
    let clients = body_json(&body);
    assert_eq!(clients["clients"].as_array().unwrap().len(), 1);
    assert_eq!(clients["clients"][0]["client_id"], "c1");

    let (s3, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        "/api/presence",
        Some(json!({ "client_id": "c1" })),
    )
    .await;
    assert_eq!(s3, StatusCode::OK);

    let (_, body2) = send_json(test_router(state), "GET", "/api/presence", None).await;
    assert_eq!(body_json(&body2)["clients"].as_array().unwrap().len(), 0);
}

// ── activity ────────────────────────────────────────────────────────

#[tokio::test]
async fn activity_feed_ok() {
    let state = test_server_state();
    let (status, body) = send_json(
        test_router(state),
        "GET",
        "/api/activity?session_id=s1",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v = body_json(&body);
    assert_eq!(v["session_id"], "s1");
    assert!(v["events"].is_array());
}

// ── missions ────────────────────────────────────────────────────────

#[tokio::test]
async fn missions_list_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/missions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["missions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn mission_create_and_get() {
    let state = test_server_state();
    let id = create_mission(&state).await;
    let (status, body) = send_json(
        test_router(state),
        "GET",
        &format!("/api/missions/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["goal"], "achieve x");
}

#[tokio::test]
async fn mission_get_not_found() {
    let state = test_server_state();
    let (status, _) = send_json(test_router(state), "GET", "/api/missions/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mission_update_ok_and_not_found() {
    let state = test_server_state();
    let id = create_mission(&state).await;
    let (s1, body) = send_json(
        test_router(state.clone()),
        "PATCH",
        &format!("/api/missions/{id}"),
        Some(json!({ "goal": "new goal", "max_iterations": 3 })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(body_json(&body)["goal"], "new goal");

    let (s2, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/missions/missing",
        Some(json!({ "goal": "x" })),
    )
    .await;
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mission_delete_ok_and_not_found() {
    let state = test_server_state();
    let id = create_mission(&state).await;
    let (s1, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        &format!("/api/missions/{id}"),
        None,
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    let (s2, _) = send_json(test_router(state), "DELETE", "/api/missions/missing", None).await;
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mission_action_start_ok() {
    let state = test_server_state();
    let id = create_mission(&state).await;
    let (status, body) = send_json(
        test_router(state),
        "POST",
        &format!("/api/missions/{id}/action"),
        Some(json!({ "action": "start" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["state"], "executing");
}

#[tokio::test]
async fn mission_action_cancel_ok() {
    let state = test_server_state();
    let id = create_mission(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        &format!("/api/missions/{id}/action"),
        Some(json!({ "action": "cancel" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn mission_action_invalid_transition_bad_request() {
    let state = test_server_state();
    let id = create_mission(&state).await;
    // Pause is not valid from the Pending state.
    let (status, _) = send_json(
        test_router(state),
        "POST",
        &format!("/api/missions/{id}/action"),
        Some(json!({ "action": "pause" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mission_action_not_found_bad_request() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/missions/missing/action",
        Some(json!({ "action": "start" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
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
