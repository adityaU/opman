//! Mock-upstream tests for the Kanban launch / abort / archive-delete paths.
//!
//! The pre-existing launch tests only cover the "upstream down → 500" and
//! no-session branches. Here we stand up a mock opencode server so the
//! **success** paths run end-to-end: `launch_task` creates a session + seeds
//! the brief, and `stop_task_agent` (via abort / archive / delete) POSTs the
//! `/session/{id}/abort` request on the awaited path.

use super::*;
use crate::web::test_support::{
    scope_base_url, send_json, start_mock_upstream, test_router, test_server_state,
};
use crate::web::types::{default_board, ServerState, Task};
use axum::http::StatusCode;
use axum::routing::post;
use serde_json::json;

fn mk_task(id: &str, board_id: &str, lane_id: &str) -> Task {
    let now = chrono::Utc::now().to_rfc3339();
    Task {
        id: id.into(),
        board_id: board_id.into(),
        lane_id: lane_id.into(),
        title: "Task".into(),
        description: "desc".into(),
        tags: vec![],
        priority: "normal".into(),
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

fn seed_board(state: &ServerState, board_id: &str) {
    let board = default_board(board_id.into(), "/tmp/gen-kh-up-proj".into());
    state
        .web_state
        .db_for_test()
        .insert_kanban_board(&board, &chrono::Utc::now().to_rfc3339());
}

/// Mock opencode that accepts session create + first message dispatch.
fn create_and_dispatch_mock(session_id: &'static str) -> axum::Router {
    axum::Router::new()
        .route(
            "/session",
            post(move || async move { axum::Json(json!({ "id": session_id })) }),
        )
        .route(
            "/session/{id}/message",
            post(|| async { axum::Json(json!({ "ok": true })) }),
        )
}

/// Mock opencode that accepts an abort POST.
fn abort_mock() -> axum::Router {
    axum::Router::new().route(
        "/session/{id}/abort",
        post(|| async { axum::Json(json!({ "ok": true })) }),
    )
}

// ── launch_task success ─────────────────────────────────────────────

#[tokio::test]
async fn launch_task_single_mode_success_records_running() {
    let base = start_mock_upstream(create_and_dispatch_mock("sess-new")).await;
    let state = test_server_state();
    seed_board(&state, "brd_up1");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_up1", "brd_up1", "lane_planning"));

    let router = test_router(state.clone());
    let (status, body) = scope_base_url(
        base,
        send_json(
            router,
            "POST",
            "/api/kanban/task/tsk_up1/launch",
            Some(json!({ "model": "sonnet", "agent": "build" })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["session_id"], "sess-new");

    // Launch metadata was recorded on the task.
    let task = state.web_state.kanban_get_task("tsk_up1").await.unwrap();
    assert_eq!(task.session_id.as_deref(), Some("sess-new"));
    assert_eq!(task.run_state, "running");
    assert_eq!(task.launch_model.as_deref(), Some("sonnet"));
    assert_eq!(task.launch_agent.as_deref(), Some("build"));
}

#[tokio::test]
async fn launch_task_success_without_model_or_agent() {
    // No model/agent in request or lane → those body fields are omitted; still
    // succeeds and records "running".
    let base = start_mock_upstream(create_and_dispatch_mock("sess-bare")).await;
    let state = test_server_state();
    seed_board(&state, "brd_up2");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_up2", "brd_up2", "lane_todo"));

    let router = test_router(state.clone());
    let (status, body) = scope_base_url(
        base,
        send_json(router, "POST", "/api/kanban/task/tsk_up2/launch", Some(json!({}))),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["session_id"], "sess-bare");
}

#[tokio::test]
async fn launch_task_session_id_missing_is_500() {
    // Create returns an object with no "id" → "session id missing" internal error.
    let mock = axum::Router::new()
        .route("/session", post(|| async { axum::Json(json!({ "no": "id" })) }));
    let base = start_mock_upstream(mock).await;
    let state = test_server_state();
    seed_board(&state, "brd_up3");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_up3", "brd_up3", "lane_todo"));

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "POST", "/api/kanban/task/tsk_up3/launch", Some(json!({}))),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn launch_task_bad_create_response_is_500() {
    // Non-JSON body from create → `.json()` parse error → internal error.
    let mock = axum::Router::new()
        .route("/session", post(|| async { "not json" }));
    let base = start_mock_upstream(mock).await;
    let state = test_server_state();
    seed_board(&state, "brd_up4");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_up4", "brd_up4", "lane_todo"));

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "POST", "/api/kanban/task/tsk_up4/launch", Some(json!({}))),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// ── abort_task success (real abort POST) ────────────────────────────

#[tokio::test]
async fn abort_task_with_session_posts_abort() {
    let base = start_mock_upstream(abort_mock()).await;
    let state = test_server_state();
    seed_board(&state, "brd_ab_up");
    let mut task = mk_task("tsk_ab_up", "brd_ab_up", "lane_todo");
    task.session_id = Some("sess-live".into());
    task.run_state = "running".into();
    state.web_state.db_for_test().insert_kanban_task(&task);

    let router = test_router(state.clone());
    let (status, body) = scope_base_url(
        base,
        send_json(router, "POST", "/api/kanban/task/tsk_ab_up/abort", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    // run_state reset to idle after abort.
    let task = state.web_state.kanban_get_task("tsk_ab_up").await.unwrap();
    assert_eq!(task.run_state, "idle");
}

// ── update_task archiving → stop_task_agent ─────────────────────────

#[tokio::test]
async fn update_task_archive_with_session_stops_agent() {
    let base = start_mock_upstream(abort_mock()).await;
    let state = test_server_state();
    seed_board(&state, "brd_arch");
    let mut task = mk_task("tsk_arch", "brd_arch", "lane_todo");
    task.session_id = Some("sess-arch".into());
    task.run_state = "running".into();
    state.web_state.db_for_test().insert_kanban_task(&task);

    let router = test_router(state.clone());
    let (status, body) = scope_base_url(
        base,
        send_json(
            router,
            "PATCH",
            "/api/kanban/task/tsk_arch",
            Some(json!({ "archived": true })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["archived"], true);
    // Archiving reset the launch state to idle.
    let task = state.web_state.kanban_get_task("tsk_arch").await.unwrap();
    assert_eq!(task.run_state, "idle");
}

// ── delete_task → stop_task_agent ───────────────────────────────────

#[tokio::test]
async fn delete_task_with_session_stops_agent() {
    let base = start_mock_upstream(abort_mock()).await;
    let state = test_server_state();
    seed_board(&state, "brd_del");
    let mut task = mk_task("tsk_del", "brd_del", "lane_todo");
    task.session_id = Some("sess-del".into());
    state.web_state.db_for_test().insert_kanban_task(&task);

    let router = test_router(state.clone());
    let (status, body) = scope_base_url(
        base,
        send_json(router, "DELETE", "/api/kanban/task/tsk_del", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    // Task row removed.
    assert!(state.web_state.kanban_get_task("tsk_del").await.is_none());
}
