//! Generated coverage tests for `web_state/kanban.rs` (part 1):
//! board resolution, task CRUD, attachments, detail, launch metadata.
use super::*;
use crate::web::types::default_board;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

/// Seed a default board directly in the DB and return its id.
fn seed_board(h: &WebStateHandle, board_id: &str, project: &str) {
    h.db_for_test()
        .insert_kanban_board(&default_board(board_id.into(), project.into()), "2026-01-01T00:00:00Z");
}

async fn make_task(h: &WebStateHandle, board_id: &str, lane: &str) -> Task {
    h.create_kanban_task(CreateTaskRequest {
        board_id: board_id.into(),
        lane_id: lane.into(),
        title: "Task".into(),
        description: "d".into(),
        tags: vec!["x".into()],
        priority: "normal".into(),
    })
    .await
    .expect("task created")
}

#[tokio::test]
async fn get_kanban_board_creates_then_reuses() {
    let h = WebStateHandle::new_test_with_projects(vec![("P".into(), PathBuf::from("/tmp/opman_kb_proj"))]);
    let first = h.get_kanban_board(Some(0)).await.expect("board");
    assert_eq!(first.board.lanes.len(), 7);
    assert!(first.tasks.is_empty());
    // Second call reuses the same board (same id).
    let second = h.get_kanban_board(Some(0)).await.unwrap();
    assert_eq!(first.board.id, second.board.id);
    // Out-of-range project index → None.
    assert!(h.get_kanban_board(Some(99)).await.is_none());
}

#[tokio::test]
async fn get_kanban_board_none_without_projects() {
    let h = WebStateHandle::new_test();
    assert!(h.get_kanban_board(None).await.is_none());
}

#[tokio::test]
async fn active_memory_scoped_to_board_project() {
    let h = WebStateHandle::new_test_with_projects(vec![("P".into(), PathBuf::from("/tmp/opman_kb_mem"))]);
    // Resolve the canonical project path the board uses.
    let board = h.get_kanban_board(Some(0)).await.unwrap().board;
    // Global memory always applies. Seed through the state mutator so it lands
    // in the in-memory cache that `list_active_memory` reads.
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "Global".into(),
        content: "c".into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    })
    .await;
    let mem = h.kanban_active_memory(&board.project_path).await;
    assert_eq!(mem.len(), 1);
    // Unknown project path → project index None, still returns global memory.
    let mem2 = h.kanban_active_memory("/no/such/path").await;
    assert_eq!(mem2.len(), 1);
}

#[tokio::test]
async fn update_board_config_found_and_missing() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let board = h.db_for_test().kanban_board("brd").unwrap();
    let new_lanes = vec![board.lanes[0].clone()];
    let updated = h
        .update_kanban_board_config("brd", new_lanes, Default::default())
        .await
        .unwrap();
    assert_eq!(updated.lanes.len(), 1);
    // Missing board id → None.
    assert!(h
        .update_kanban_board_config("ghost", vec![], Default::default())
        .await
        .is_none());
}

#[tokio::test]
async fn create_task_missing_board_is_none() {
    let h = WebStateHandle::new_test();
    let r = h
        .create_kanban_task(CreateTaskRequest {
            board_id: "nope".into(),
            lane_id: "lane_todo".into(),
            title: "t".into(),
            description: String::new(),
            tags: vec![],
            priority: "normal".into(),
        })
        .await;
    assert!(r.is_none());
}

#[tokio::test]
async fn create_task_appends_order_index() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t1 = make_task(&h, "brd", "lane_todo").await;
    let t2 = make_task(&h, "brd", "lane_todo").await;
    assert!(t2.order_index > t1.order_index);
    assert_eq!(t1.run_state, "idle");
}

#[tokio::test]
async fn update_task_not_found_and_forbidden_and_ok() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;

    // Not found.
    assert!(matches!(
        h.update_kanban_task("ghost", UpdateTaskRequest {
            title: None, description: None, tags: None, priority: None,
            lane_id: None, order_index: None, archived: None,
        }).await,
        Err(KanbanError::NotFound)
    ));

    // Illegal transition todo → done.
    assert!(matches!(
        h.update_kanban_task(&t.id, UpdateTaskRequest {
            title: None, description: None, tags: None, priority: None,
            lane_id: Some("lane_done".into()), order_index: None, archived: None,
        }).await,
        Err(KanbanError::Forbidden(_))
    ));

    // Legal move + full field edits.
    let updated = h.update_kanban_task(&t.id, UpdateTaskRequest {
        title: Some("New".into()),
        description: Some("newdesc".into()),
        tags: Some(vec!["a".into(), "b".into()]),
        priority: Some("high".into()),
        lane_id: Some("lane_planning".into()),
        order_index: Some(42.0),
        archived: Some(true),
    }).await.unwrap();
    assert_eq!(updated.title, "New");
    assert_eq!(updated.description, "newdesc");
    assert_eq!(updated.tags, vec!["a", "b"]);
    assert_eq!(updated.priority, "high");
    assert_eq!(updated.lane_id, "lane_planning");
    assert_eq!(updated.order_index, 42.0);
    assert!(updated.archived);
}

#[tokio::test]
async fn update_task_same_lane_move_allowed() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // Moving to the same lane is always allowed (reorder).
    let updated = h.update_kanban_task(&t.id, UpdateTaskRequest {
        title: None, description: None, tags: None, priority: None,
        lane_id: Some("lane_todo".into()), order_index: Some(9.0), archived: None,
    }).await.unwrap();
    assert_eq!(updated.lane_id, "lane_todo");
}

#[tokio::test]
async fn delete_task_missing_and_present() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    assert!(!h.delete_kanban_task("ghost").await);
    let t = make_task(&h, "brd", "lane_todo").await;
    assert!(h.delete_kanban_task(&t.id).await);
    assert!(h.kanban_get_task(&t.id).await.is_none());
}

#[tokio::test]
async fn task_detail_none_and_with_notes_attachments() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    assert!(h.get_kanban_task_detail("ghost").await.is_none());

    let t = make_task(&h, "brd", "lane_todo").await;
    h.kanban_internal_note(&t.id, "progress", None, None).await.unwrap();
    let att = h.add_kanban_attachment(&t.id, "pic.png", "image/png", 123).await.unwrap();
    assert_eq!(att.kind, "image");
    assert_eq!(att.url, "/api/kanban/asset/".to_string() + &t.id + "/pic.png");

    let detail = h.get_kanban_task_detail(&t.id).await.unwrap();
    assert_eq!(detail.notes.len(), 1);
    assert_eq!(detail.attachments.len(), 1);
    assert!(detail.attachments[0].url.contains("pic.png"));
}

#[tokio::test]
async fn add_attachment_missing_task_is_none() {
    let h = WebStateHandle::new_test();
    assert!(h.add_kanban_attachment("ghost", "f.bin", "application/octet-stream", 1).await.is_none());
}

#[tokio::test]
async fn set_task_launch_missing_and_ok() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    assert!(h.set_kanban_task_launch("ghost", None, None, None, "running").await.is_none());

    let t = make_task(&h, "brd", "lane_todo").await;
    let launched = h.set_kanban_task_launch(
        &t.id,
        Some("sess-1".into()),
        Some("claude".into()),
        Some("build".into()),
        "running",
    ).await.unwrap();
    assert_eq!(launched.session_id.as_deref(), Some("sess-1"));
    assert_eq!(launched.launch_model.as_deref(), Some("claude"));
    assert_eq!(launched.launch_agent.as_deref(), Some("build"));
    assert_eq!(launched.run_state, "running");
}

#[tokio::test]
async fn get_task_and_board_accessors() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    assert!(h.kanban_get_task(&t.id).await.is_some());
    assert!(h.kanban_get_task("ghost").await.is_none());
    assert!(h.kanban_get_board("brd").await.is_some());
    assert!(h.kanban_get_board("ghost").await.is_none());
}
