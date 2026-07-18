//! Generated tests for the loopback internal Kanban *query* API
//! (`/internal/kanban/task/{id}/query|board|notes`).

use super::*;
use crate::web::test_support::{test_router, test_server_state};
use crate::web::types::{default_board, KanbanNote, ServerState, Task};
use axum::http::StatusCode;
use serde_json::{json, Value};

const TOKEN: &str = "test-internal-token";

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

fn add_task(
    state: &ServerState,
    id: &str,
    board_id: &str,
    lane: &str,
    tags: &[&str],
    archived: bool,
    title: &str,
    desc: &str,
) {
    let now = chrono::Utc::now().to_rfc3339();
    let task = Task {
        id: id.into(),
        board_id: board_id.into(),
        lane_id: lane.into(),
        title: title.into(),
        description: desc.into(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        priority: "normal".into(),
        order_index: 1.0,
        session_id: None,
        launch_model: None,
        launch_agent: None,
        run_state: "idle".into(),
        archived,
        created_at: now.clone(),
        updated_at: now,
    };
    state.web_state.db_for_test().insert_kanban_task(&task);
}

/// Seed a board with a mix of tasks; returns the anchor task id.
fn seed(state: &ServerState) -> String {
    let db = state.web_state.db_for_test();
    let board = default_board("brd_q".into(), "/tmp/gen-q-proj".into());
    let now = chrono::Utc::now().to_rfc3339();
    db.insert_kanban_board(&board, &now);
    add_task(state, "tsk_anchor", &board.id, "lane_todo", &["backend", "urgent"], false, "Anchor task", "fix the parser");
    add_task(state, "tsk_two", &board.id, "lane_planning", &["frontend"], false, "Second", "build UI");
    add_task(state, "tsk_arch", &board.id, "lane_todo", &["backend"], true, "Archived one", "old work");
    "tsk_anchor".to_string()
}

// ── internal_query_tasks ────────────────────────────────────────────

#[tokio::test]
async fn query_all_active_default() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    // Archived excluded by default → 2 active tasks.
    assert_eq!(v["count"], 2);
    assert!(v["tasks"].is_array());
}

#[tokio::test]
async fn query_include_archived() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({ "include_archived": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["count"], 3);
}

#[tokio::test]
async fn query_filter_by_lane_name() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({ "lane": "Planning" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["count"], 1);
    assert_eq!(v["tasks"][0]["title"], "Second");
    assert_eq!(v["tasks"][0]["lane"], "Planning");
}

#[tokio::test]
async fn query_empty_lane_string_is_ignored() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({ "lane": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["count"], 2);
}

#[tokio::test]
async fn query_filter_by_tags() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({ "tags": ["FRONTEND"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["count"], 1);
    assert_eq!(v["tasks"][0]["title"], "Second");
}

#[tokio::test]
async fn query_filter_by_text() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({ "query": "parser" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["count"], 1);
    assert_eq!(v["tasks"][0]["id"], "tsk_anchor");
}

#[tokio::test]
async fn query_unknown_lane_is_409() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some(TOKEN),
        Some(json!({ "lane": "Bogus" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn query_anchor_not_found_is_404() {
    let state = test_server_state();
    seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        "/internal/kanban/task/missing/query",
        Some(TOKEN),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn query_bad_token_is_401() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/query"),
        Some("bad"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── internal_board_overview ─────────────────────────────────────────

#[tokio::test]
async fn board_overview_ok() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, body) = send_tok(
        test_router(state),
        "GET",
        &format!("/internal/kanban/task/{anchor}/board"),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["board_id"], "brd_q");
    assert_eq!(v["total_active"], 2);
    let lanes = v["lanes"].as_array().unwrap();
    // Find the Todo lane: 1 active + 1 archived.
    let todo = lanes
        .iter()
        .find(|l| l["id"] == "lane_todo")
        .expect("todo lane present");
    assert_eq!(todo["active_count"], 1);
    assert_eq!(todo["archived_count"], 1);
    assert!(todo["next_lanes"].is_array());
}

#[tokio::test]
async fn board_overview_anchor_not_found_is_404() {
    let state = test_server_state();
    seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        "/internal/kanban/task/missing/board",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn board_overview_missing_token_is_401() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "GET",
        &format!("/internal/kanban/task/{anchor}/board"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── internal_read_notes ─────────────────────────────────────────────

#[tokio::test]
async fn read_notes_defaults_to_anchor() {
    let state = test_server_state();
    let anchor = seed(&state);
    // Add a note to the anchor.
    state.web_state.db_for_test().insert_kanban_note(
        &KanbanNote {
            id: "nte_1".into(),
            author: "agent".into(),
            body: "did a thing".into(),
            lane_from: Some("lane_todo".into()),
            lane_to: Some("lane_planning".into()),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        &anchor,
    );
    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/notes"),
        Some(TOKEN),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    let tasks = v["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "tsk_anchor");
    assert_eq!(tasks[0]["note_count"], 1);
    assert_eq!(tasks[0]["notes"][0]["body"], "did a thing");
}

#[tokio::test]
async fn read_notes_explicit_ids_skips_unknown_and_offboard() {
    let state = test_server_state();
    let anchor = seed(&state);
    // A task on a *different* board — must be skipped (board isolation).
    let other = default_board("brd_other".into(), "/tmp/other".into());
    state
        .web_state
        .db_for_test()
        .insert_kanban_board(&other, &chrono::Utc::now().to_rfc3339());
    add_task(&state, "tsk_offboard", &other.id, "lane_todo", &[], false, "Off", "x");

    let (status, body) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/notes"),
        Some(TOKEN),
        Some(json!({ "task_ids": ["tsk_two", "tsk_offboard", "ghost"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    let tasks = v["tasks"].as_array().unwrap();
    // Only tsk_two is on-board and exists; offboard + ghost are skipped.
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "tsk_two");
    assert_eq!(tasks[0]["note_count"], 0);
}

#[tokio::test]
async fn read_notes_anchor_not_found_is_404() {
    let state = test_server_state();
    seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        "/internal/kanban/task/missing/notes",
        Some(TOKEN),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn read_notes_bad_token_is_401() {
    let state = test_server_state();
    let anchor = seed(&state);
    let (status, _) = send_tok(
        test_router(state),
        "POST",
        &format!("/internal/kanban/task/{anchor}/notes"),
        Some("bad"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
