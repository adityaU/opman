//! Generated tests (part 2) for the Kanban HTTP handlers: launch / abort /
//! user-note endpoints, asset serving, and multipart attachment upload.

use super::*;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::{default_board, ServerState, Task};
use axum::http::{header, StatusCode};

// ── fixtures ────────────────────────────────────────────────────────

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
    let board = default_board(board_id.into(), "/tmp/gen-kh2-proj".into());
    state
        .web_state
        .db_for_test()
        .insert_kanban_board(&board, &chrono::Utc::now().to_rfc3339());
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}_{}", rand::random::<u64>())
}

/// GET with an optional Range header.
async fn get_range(
    router: axum::Router,
    uri: &str,
    range: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let mut b = Request::builder().method("GET").uri(uri);
    if let Some(r) = range {
        b = b.header(header::RANGE, r);
    }
    let resp = router.oneshot(b.body(Body::empty()).unwrap()).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}

/// POST a raw multipart/form-data body.
async fn post_multipart(
    router: axum::Router,
    uri: &str,
    boundary: &str,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let ct = format!("multipart/form-data; boundary={boundary}");
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, ct)
        .body(Body::from(body))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

// ════════════════════════════════════════════════════════════════════
// launch_task
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn launch_task_not_found_is_404() {
    let (status, _) = send_json(
        test_router(test_server_state()),
        "POST",
        "/api/kanban/task/ghost/launch",
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn launch_task_already_running_is_400() {
    let state = test_server_state();
    seed_board(&state, "brd_lr");
    let mut task = mk_task("tsk_lr", "brd_lr", "lane_todo");
    task.run_state = "running".into();
    state.web_state.db_for_test().insert_kanban_task(&task);
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_lr/launch",
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn launch_task_board_missing_is_404() {
    let state = test_server_state();
    // Task exists but its board does not.
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_nob", "ghost_board", "lane_todo"));
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_nob/launch",
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn launch_task_single_mode_upstream_down_is_500() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:1".to_string());
    let state = test_server_state();
    seed_board(&state, "brd_ls");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_ls", "brd_ls", "lane_planning"));
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_ls/launch",
        Some(serde_json::json!({ "model": "sonnet", "agent": "build" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn launch_task_pipeline_mode_is_400() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:1".to_string());
    let state = test_server_state();
    seed_board(&state, "brd_lp");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_lp", "brd_lp", "lane_todo"));
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_lp/launch",
        Some(serde_json::json!({ "mode": "pipeline" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ════════════════════════════════════════════════════════════════════
// abort_task
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn abort_task_not_found_is_404() {
    let (status, _) = send_json(
        test_router(test_server_state()),
        "POST",
        "/api/kanban/task/ghost/abort",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn abort_task_no_session_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_ab");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_ab", "brd_ab", "lane_todo"));
    let (status, body) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_ab/abort",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn abort_task_with_session_ok() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:1".to_string());
    let state = test_server_state();
    seed_board(&state, "brd_abs");
    let mut task = mk_task("tsk_abs", "brd_abs", "lane_todo");
    task.session_id = Some("sess-z".into());
    task.run_state = "running".into();
    state.web_state.db_for_test().insert_kanban_task(&task);
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_abs/abort",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ════════════════════════════════════════════════════════════════════
// add_user_note
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn add_user_note_empty_body_is_400() {
    let state = test_server_state();
    seed_board(&state, "brd_ne");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_ne", "brd_ne", "lane_todo"));
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_ne/note",
        Some(serde_json::json!({ "body": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_user_note_ok() {
    let state = test_server_state();
    seed_board(&state, "brd_nk");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task("tsk_nk", "brd_nk", "lane_todo"));
    let (status, body) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_nk/note",
        Some(serde_json::json!({ "body": "please review" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["author"], "user");
    assert_eq!(v["body"], "please review");
}

#[tokio::test]
async fn add_user_note_live_session_delivers() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:1".to_string());
    let state = test_server_state();
    seed_board(&state, "brd_nl");
    let mut task = mk_task("tsk_nl", "brd_nl", "lane_todo");
    task.session_id = Some("sess-live".into());
    task.run_state = "running".into();
    state.web_state.db_for_test().insert_kanban_task(&task);
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/kanban/task/tsk_nl/note",
        Some(serde_json::json!({ "body": "mid-flight note" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn add_user_note_task_not_found_is_404() {
    let (status, _) = send_json(
        test_router(test_server_state()),
        "POST",
        "/api/kanban/task/ghost/note",
        Some(serde_json::json!({ "body": "hi" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ════════════════════════════════════════════════════════════════════
// serve_asset
// ════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn serve_asset_missing_dir_is_404() {
    let id = unique_id("gen_noexist");
    let (status, _, _) = get_range(
        test_router(test_server_state()),
        &format!("/api/kanban/asset/{id}/nope.png"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn serve_asset_full_and_range() {
    let id = unique_id("gen_asset");
    let dir = assets_dir(&id);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("clip.mp4");
    std::fs::write(&file, b"0123456789").unwrap();

    let router = test_router(test_server_state());
    let uri = format!("/api/kanban/asset/{id}/clip.mp4");

    // Full body.
    let (status, headers, body) = get_range(router.clone(), &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "video/mp4");
    assert_eq!(body, b"0123456789");

    // Ranged body (bytes 2..=5).
    let (status, headers, body) = get_range(router.clone(), &uri, Some("bytes=2-5")).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, b"2345");
    assert_eq!(
        headers.get(header::CONTENT_RANGE).unwrap(),
        "bytes 2-5/10"
    );

    // Invalid range falls through to the full 200 response.
    let (status, _, body) = get_range(router, &uri, Some("bytes=garbage")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"0123456789");

    let _ = std::fs::remove_dir_all(&dir);
}

// ════════════════════════════════════════════════════════════════════
// upload_attachment
// ════════════════════════════════════════════════════════════════════

fn multipart_body(boundary: &str, field: &str, filename: &str, ct: &str, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    out.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    out.extend_from_slice(format!("Content-Type: {ct}\r\n\r\n").as_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    out
}

#[tokio::test]
async fn upload_task_not_found_is_404() {
    let boundary = "XB";
    let body = multipart_body(boundary, "file", "a.png", "image/png", b"hi");
    let (status, _) = post_multipart(
        test_router(test_server_state()),
        "/api/kanban/task/ghost/attachment",
        boundary,
        body,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upload_no_file_field_is_400() {
    let state = test_server_state();
    seed_board(&state, "brd_uf");
    let id = unique_id("tsk_uf");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task(&id, "brd_uf", "lane_todo"));
    let boundary = "YB";
    // A field named something other than "file".
    let body = multipart_body(boundary, "other", "a.txt", "text/plain", b"x");
    let (status, _) = post_multipart(
        test_router(state),
        &format!("/api/kanban/task/{id}/attachment"),
        boundary,
        body,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_ok_writes_attachment() {
    let state = test_server_state();
    seed_board(&state, "brd_uok");
    let id = unique_id("tsk_uok");
    state
        .web_state
        .db_for_test()
        .insert_kanban_task(&mk_task(&id, "brd_uok", "lane_todo"));
    let boundary = "ZB";
    let body = multipart_body(boundary, "file", "shot.png", "image/png", b"PNGDATA");
    let (status, resp) = post_multipart(
        test_router(state),
        &format!("/api/kanban/task/{id}/attachment"),
        boundary,
        body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp).unwrap();
    assert_eq!(v["filename"], "shot.png");
    assert_eq!(v["kind"], "image");
    assert_eq!(v["size_bytes"], 7);

    let _ = std::fs::remove_dir_all(assets_dir(&id));
}
