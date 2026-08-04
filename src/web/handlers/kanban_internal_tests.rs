//! Generated tests for the loopback internal Kanban mutation API
//! (`/internal/kanban/task/*`). These routes are guarded by the
//! `X-Internal-Token` header instead of the `AuthUser` extractor.

use super::*;
use crate::web::test_support::{test_router, test_server_state};
use crate::web::types::{default_board, Attachment, ServerState, Task};
use axum::http::StatusCode;
use serde_json::{json, Value};

const TOKEN: &str = "test-internal-token";

/// Send a request with an optional `x-internal-token` header.
async fn send_tok(
    router: axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("x-internal-token", t);
    }
    let req = match body {
        Some(v) => b.body(Body::from(serde_json::to_vec(&v).unwrap())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

fn mk_task(id: &str, board_id: &str, lane_id: &str) -> Task {
    let now = chrono::Utc::now().to_rfc3339();
    Task {
        id: id.into(),
        board_id: board_id.into(),
        lane_id: lane_id.into(),
        title: "My Task".into(),
        description: "the description".into(),
        tags: vec!["backend".into()],
        priority: "high".into(),
        order_index: 1.0,
        session_id: None,
        launch_model: None,
        launch_agent: None,
        run_state: "idle".into(),
        archived: false,
        created_at: now.clone(),
        updated_at: now,
    }
}

/// Seed a default board + one task in `lane`, returning the task id.
fn seed(state: &ServerState, lane: &str) -> String {
    let db = state.web_state.db_for_test();
    let board = default_board("brd_gen".into(), "/tmp/gen-int-proj".into());
    let now = chrono::Utc::now().to_rfc3339();
    db.insert_kanban_board(&board, &now);
    let task = mk_task("tsk_gen1", &board.id, lane);
    db.insert_kanban_task(&task);
    task.id
}

// ── check_internal_token ────────────────────────────────────────────

#[tokio::test]
async fn get_task_missing_token_is_401() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        &format!("/internal/kanban/task/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_task_wrong_token_is_401() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        &format!("/internal/kanban/task/{id}"),
        Some("nope"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn empty_server_token_rejects_even_matching() {
    // When the server's configured token is empty, EVERY request is rejected.
    let mut state = test_server_state();
    state.internal_token = String::new();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        &format!("/internal/kanban/task/{id}"),
        Some(""),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── internal_get_task ───────────────────────────────────────────────

#[tokio::test]
async fn get_task_ok_full_payload() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    // Attach an asset so the attachments-mapping closure is exercised.
    state
        .web_state
        .db_for_test()
        .insert_kanban_attachment(&Attachment {
            id: "att_1".into(),
            task_id: id.clone(),
            filename: "shot.png".into(),
            mime: "image/png".into(),
            kind: "image".into(),
            size_bytes: 10,
            created_at: chrono::Utc::now().to_rfc3339(),
            url: String::new(),
        });
    let (status, body) = send_tok(
        test_router(state),
        "GET",
        &format!("/internal/kanban/task/{id}"),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["id"], id);
    assert_eq!(v["title"], "My Task");
    assert_eq!(v["priority"], "high");
    assert!(v["allowed_transitions"].is_array());
    assert!(v["current_lane"].is_object());
    assert_eq!(v["terminal_lane"]["name"], "In Review");
    assert_eq!(v["attachments"][0]["filename"], "shot.png");
    // lane_todo → allowed to move to Planning (forward edge).
    let names: Vec<String> = v["allowed_transitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Planning".to_string()));
}

#[tokio::test]
async fn get_task_not_found_is_404() {
    let state = test_server_state();
    seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        "/internal/kanban/task/does-not-exist",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_task_board_missing_is_404() {
    // Task exists but references a board that was never inserted.
    let state = test_server_state();
    let db = state.web_state.db_for_test();
    db.insert_kanban_task(&mk_task("tsk_orphan", "ghost_board", "lane_todo"));
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        "/internal/kanban/task/tsk_orphan",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── internal_set_status ─────────────────────────────────────────────

#[tokio::test]
async fn set_status_allowed_by_name_ok() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/status"),
        Some(TOKEN),
        Some(json!({ "lane": "Planning", "run_state": "running" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["lane_id"], "lane_planning");
}

#[tokio::test]
async fn set_status_invalid_transition_is_409() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    // Todo may only reach Planning; Done is not adjacent.
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/status"),
        Some(TOKEN),
        Some(json!({ "lane": "lane_done" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn set_status_unknown_lane_is_409() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/status"),
        Some(TOKEN),
        Some(json!({ "lane": "Nonexistent" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn set_status_task_not_found_is_404() {
    let state = test_server_state();
    seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        "/internal/kanban/task/missing/status",
        Some(TOKEN),
        Some(json!({ "lane": "Planning" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn set_status_bad_token_is_401() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/status"),
        Some("bad"),
        Some(json!({ "lane": "Planning" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── internal_add_note ───────────────────────────────────────────────

#[tokio::test]
async fn add_note_ok() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/note"),
        Some(TOKEN),
        Some(json!({ "body": "progress!", "lane_from": "lane_todo", "lane_to": "lane_planning" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn add_note_task_not_found_is_404() {
    let state = test_server_state();
    seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        "/internal/kanban/task/missing/note",
        Some(TOKEN),
        Some(json!({ "body": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_note_bad_token_is_401() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/note"),
        None,
        Some(json!({ "body": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── internal_complete ───────────────────────────────────────────────

#[tokio::test]
async fn complete_moves_to_terminal_with_summary() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/complete"),
        Some(TOKEN),
        Some(json!({ "body": "all done and tested" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["lane_id"], "lane_inreview");
}

#[tokio::test]
async fn complete_empty_summary_ok() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/complete"),
        Some(TOKEN),
        Some(json!({ "body": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn complete_task_not_found_is_404() {
    let state = test_server_state();
    seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        "/internal/kanban/task/missing/complete",
        Some(TOKEN),
        Some(json!({ "body": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn complete_bad_token_is_401() {
    let state = test_server_state();
    let id = seed(&state, "lane_todo");
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{id}/complete"),
        Some("wrong"),
        Some(json!({ "body": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
