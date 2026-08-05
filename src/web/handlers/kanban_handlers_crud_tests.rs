//! Generated tests (part 1) for the Kanban HTTP handlers: pure helpers plus
//! board/task CRUD endpoints driven through the production router.

use super::*;
use crate::web::error::WebError;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::{default_board, MemoryScope, PersonalMemoryItem, ServerState, Task};
use crate::web::web_state::{KanbanError, WebStateHandle};
use axum::http::StatusCode;
use serde_json::json;

// ── test fixtures ───────────────────────────────────────────────────

fn mk_task(id: &str, board_id: &str, lane_id: &str) -> Task {
    let now = chrono::Utc::now().to_rfc3339();
    Task {
        id: id.into(),
        board_id: board_id.into(),
        lane_id: lane_id.into(),
        title: "Task title".into(),
        description: "Task description".into(),
        tags: vec!["x".into()],
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
    let board = default_board(board_id.into(), "/tmp/gen-kh-proj".into());
    state
        .web_state
        .db_for_test()
        .insert_kanban_board(&board, &chrono::Utc::now().to_rfc3339());
}

// ════════════════════════════════════════════════════════════════════
// Pure helpers
// ════════════════════════════════════════════════════════════════════

// ── sanitize_filename ───────────────────────────────────────────────

#[test]
fn sanitize_strips_separators_and_traversal() {
    assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    // ".." → "_", then each '/' → '_'.
    assert_eq!(sanitize_filename("../../etc/passwd"), "____etc_passwd");
    assert_eq!(sanitize_filename("....//x"), "____x");
}

#[test]
fn sanitize_leading_dots_and_empty() {
    // "..hidden" → ".." becomes "_", leaving "_.hidden" (leading '_' blocks the
    // dot-trim), so the result keeps the underscore.
    assert_eq!(sanitize_filename("...hidden"), "_.hidden");
    assert_eq!(sanitize_filename(""), "file");
    // A name that reduces to empty after stripping falls back to "file".
    assert_eq!(sanitize_filename("."), "file");
}

#[test]
fn sanitize_plain_name_unchanged() {
    assert_eq!(sanitize_filename("photo.png"), "photo.png");
}

// ── parse_range ─────────────────────────────────────────────────────

#[test]
fn parse_range_basic() {
    assert_eq!(parse_range("bytes=0-4", 10), Some((0, 4)));
}

#[test]
fn parse_range_open_ended() {
    assert_eq!(parse_range("bytes=5-", 10), Some((5, 9)));
}

#[test]
fn parse_range_clamps_end() {
    assert_eq!(parse_range("bytes=2-999", 10), Some((2, 9)));
}

#[test]
fn parse_range_zero_total_none() {
    assert_eq!(parse_range("bytes=0-4", 0), None);
}

#[test]
fn parse_range_bad_inputs() {
    assert_eq!(parse_range("0-4", 10), None); // no prefix
    assert_eq!(parse_range("bytes=abc", 10), None); // no dash
    assert_eq!(parse_range("bytes=x-y", 10), None); // non-numeric start
    assert_eq!(parse_range("bytes=1-z", 10), None); // non-numeric end
    assert_eq!(parse_range("bytes=8-3", 10), None); // start > end
}

// ── guess_mime ──────────────────────────────────────────────────────

#[test]
fn guess_mime_known() {
    assert_eq!(guess_mime("a.png"), "image/png");
    assert_eq!(guess_mime("a.JPG"), "image/jpeg");
    assert_eq!(guess_mime("a.jpeg"), "image/jpeg");
    assert_eq!(guess_mime("a.gif"), "image/gif");
    assert_eq!(guess_mime("a.webp"), "image/webp");
    assert_eq!(guess_mime("a.svg"), "image/svg+xml");
    assert_eq!(guess_mime("a.mp4"), "video/mp4");
    assert_eq!(guess_mime("a.webm"), "video/webm");
    assert_eq!(guess_mime("a.mov"), "video/quicktime");
    assert_eq!(guess_mime("a.pdf"), "application/pdf");
    assert_eq!(guess_mime("a.txt"), "text/plain");
    assert_eq!(guess_mime("a.md"), "text/plain");
}

#[test]
fn guess_mime_unknown_and_no_ext() {
    assert_eq!(guess_mime("a.xyz"), "application/octet-stream");
    assert_eq!(guess_mime("noext"), "application/octet-stream");
}

// ── map_kanban_err ──────────────────────────────────────────────────

#[test]
fn map_kanban_err_variants() {
    match map_kanban_err(KanbanError::NotFound) {
        WebError::NotFound("task") => {}
        other => panic!("expected NotFound(task), got {other:?}"),
    }
    match map_kanban_err(KanbanError::Forbidden("nope".into())) {
        WebError::Upstream(code, msg) => {
            assert_eq!(code, StatusCode::CONFLICT);
            assert_eq!(msg, "nope");
        }
        other => panic!("expected Upstream, got {other:?}"),
    }
}

// ── inject_memory_guidance ──────────────────────────────────────────

fn mem_item(label: &str, content: &str) -> PersonalMemoryItem {
    let now = chrono::Utc::now().to_rfc3339();
    PersonalMemoryItem {
        id: "mem1".into(),
        label: label.into(),
        content: content.into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
        created_at: now.clone(),
        updated_at: now,
    }
}

#[test]
fn inject_memory_empty_returns_unchanged() {
    let out = inject_memory_guidance("the brief", &[]);
    assert_eq!(out, "the brief");
}

#[test]
fn inject_memory_prepends_guidance() {
    let mem = vec![mem_item("Style", "be terse"), mem_item("Lang", "rust")];
    let out = inject_memory_guidance("do the thing", &mem);
    assert!(out.starts_with("[Session instructions]"));
    assert!(out.contains("- Style: be terse"));
    assert!(out.contains("- Lang: rust"));
    assert!(out.contains("[User request]"));
    assert!(out.contains("do the thing"));
}

// ── build_brief ─────────────────────────────────────────────────────

#[test]
fn build_brief_with_default_board() {
    let board = default_board("brd".into(), "/tmp/p".into());
    let task = mk_task("tsk1", "brd", "lane_todo");
    let brief = build_brief(&task, &board);
    assert!(brief.contains("TASK: Task title"));
    assert!(brief.contains("TAGS: x"));
    assert!(brief.contains("PRIORITY: normal"));
    assert!(brief.contains("CURRENT LANE: Todo"));
    // Todo → Planning is the forward transition.
    assert!(brief.contains("YOU MAY MOVE TO: Planning"));
    // terminal lane name surfaces in the completion instruction.
    assert!(brief.contains("In Review"));
    assert!(brief.contains("task_id=\"tsk1\""));
}

#[test]
fn build_brief_empty_tags_and_no_transitions() {
    // A board with a single lane, no transitions, no terminal lane.
    let board = default_board("brd".into(), "/tmp/p".into());
    let mut board = board;
    board.transitions.clear();
    board.lanes.retain(|l| l.id == "lane_todo");
    let mut task = mk_task("tsk2", "brd", "lane_todo");
    task.tags.clear();
    let brief = build_brief(&task, &board);
    assert!(brief.contains("TAGS: (none)"));
    assert!(brief.contains("YOU MAY MOVE TO: (none)"));
    // No terminal lane → fallback "In Review".
    assert!(brief.contains("In Review"));
}

#[test]
fn build_brief_unknown_current_lane_falls_back_to_id() {
    let board = default_board("brd".into(), "/tmp/p".into());
    let task = mk_task("tsk3", "brd", "lane_ghost");
    let brief = build_brief(&task, &board);
    assert!(brief.contains("CURRENT LANE: lane_ghost"));
}

// ── assets_dir ──────────────────────────────────────────────────────

#[test]
fn assets_dir_contains_sanitized_task_id() {
    let dir = assets_dir("tsk/../evil");
    let s = dir.to_string_lossy();
    assert!(s.contains("kanban"));
    assert!(s.contains("assets"));
    assert!(!s.contains(".."));
}

// ════════════════════════════════════════════════════════════════════
// Board endpoints
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn get_board_no_active_project_is_400() {
    let (status, _) = send_json(
        test_router(test_server_state()),
        "GET",
        "/api/kanban/board",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_board_with_project_ok() {
    let mut state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("demo".into(), tmp.path().to_path_buf())]);
    let (status, body) = send_json(test_router(state), "GET", "/api/kanban/board", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["board"]["lanes"].is_array());
    assert!(v["tasks"].is_array());
    drop(tmp);
}

#[tokio::test]
async fn get_board_with_pi_query_ok() {
    let mut state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("demo".into(), tmp.path().to_path_buf())]);
    let (status, _) = send_json(test_router(state), "GET", "/api/kanban/board?pi=0", None).await;
    assert_eq!(status, StatusCode::OK);
    drop(tmp);
}

#[tokio::test]
async fn update_board_config_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_cfg");
    let body = json!({
        "lanes": [
            { "id": "l1", "name": "One", "color": "#111" },
            { "id": "l2", "name": "Two", "color": "#222", "terminal": true }
        ],
        "transitions": { "l1": ["l2"], "l2": [] }
    });
    let (status, resp) = send_json(
        test_router(state),
        "PUT",
        "/api/kanban/board/brd_cfg/config",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp).unwrap();
    assert_eq!(v["board"]["lanes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn update_board_config_missing_board_is_404() {
    let body = json!({ "lanes": [], "transitions": {} });
    let (status, _) = send_json(
        test_router(test_server_state()),
        "PUT",
        "/api/kanban/board/ghost/config",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════
// Task CRUD endpoints
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn create_task_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_new");
    let body = json!({
        "board_id": "brd_new",
        "lane_id": "lane_todo",
        "title": "Fresh task",
        "tags": ["a", "b"],
        "priority": "high"
    });
    let (status, resp) =
        send_json(test_router(state), "POST", "/api/kanban/task", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp).unwrap();
    assert_eq!(v["title"], "Fresh task");
    assert_eq!(v["lane_id"], "lane_todo");
}

#[tokio::test]
async fn create_task_unknown_board_is_404() {
    let body = json!({ "board_id": "ghost", "lane_id": "lane_todo", "title": "x" });
    let (status, _) = send_json(
        test_router(test_server_state()),
        "POST",
        "/api/kanban/task",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_task_ok_and_missing() {
    let state = test_server_state();
    seed_board(&state, "brd_g");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_g", "brd_g", "lane_todo"));
    let router = test_router(state);
    let (status, body) = send_json(router.clone(), "GET", "/api/kanban/task/tsk_g", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["id"], "tsk_g");
    // TaskDetail flattens task + notes + attachments.
    assert!(v["notes"].is_array());

    let (status, _) = send_json(router, "GET", "/api/kanban/task/missing", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_task_edit_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_u");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_u", "brd_u", "lane_todo"));
    let body =
        json!({ "title": "renamed", "description": "d2", "priority": "low", "order_index": 3.5 });
    let (status, resp) = send_json(
        test_router(state),
        "PATCH",
        "/api/kanban/task/tsk_u",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp).unwrap();
    assert_eq!(v["title"], "renamed");
    assert_eq!(v["priority"], "low");
}

#[tokio::test]
async fn update_task_valid_move_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_mv");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_mv", "brd_mv", "lane_todo"));
    let body = json!({ "lane_id": "lane_planning" });
    let (status, resp) = send_json(
        test_router(state),
        "PATCH",
        "/api/kanban/task/tsk_mv",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp).unwrap();
    assert_eq!(v["lane_id"], "lane_planning");
}

#[tokio::test]
async fn update_task_invalid_transition_is_409() {
    let state = test_server_state();
    seed_board(&state, "brd_bad");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_bad", "brd_bad", "lane_todo"));
    let body = json!({ "lane_id": "lane_done" });
    let (status, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/kanban/task/tsk_bad",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn update_task_missing_is_404() {
    let body = json!({ "title": "x" });
    let (status, _) = send_json(
        test_router(test_server_state()),
        "PATCH",
        "/api/kanban/task/ghost",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn archive_task_no_session_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_ar");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_ar", "brd_ar", "lane_todo"));
    let body = json!({ "archived": true });
    let (status, resp) = send_json(
        test_router(state),
        "PATCH",
        "/api/kanban/task/tsk_ar",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp).unwrap();
    assert_eq!(v["archived"], true);
}

#[tokio::test]
async fn archive_task_with_session_stops_agent() {
    // Exercises the stop_task_agent Some(session) branch + pipeline stop.
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:1".to_string());
    let state = test_server_state();
    seed_board(&state, "brd_ars");
    let mut task = mk_task("tsk_ars", "brd_ars", "lane_todo");
    task.session_id = Some("sess-x".into());
    task.run_state = "running".into();
    state.web_state.db_for_test().insert_kanban_task(&task);
    let body = json!({ "archived": true });
    let (status, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/kanban/task/tsk_ars",
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn delete_task_ok_and_missing() {
    let state = test_server_state();
    seed_board(&state, "brd_d");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_d", "brd_d", "lane_todo"));
    let router = test_router(state);
    let (status, body) = send_json(router.clone(), "DELETE", "/api/kanban/task/tsk_d", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);

    let (status, _) = send_json(router, "DELETE", "/api/kanban/task/tsk_d", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_task_with_session_stops_agent() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:1".to_string());
    let state = test_server_state();
    seed_board(&state, "brd_ds");
    let mut task = mk_task("tsk_ds", "brd_ds", "lane_todo");
    task.session_id = Some("sess-y".into());
    state.web_state.db_for_test().insert_kanban_task(&task);
    let (status, _) = send_json(
        test_router(state),
        "DELETE",
        "/api/kanban/task/tsk_ds",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
