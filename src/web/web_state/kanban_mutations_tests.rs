//! Generated coverage tests for `web_state/kanban.rs` (part 2):
//! MCP-facing lane moves, completion, notes, and user notes.
use super::*;
use crate::web::types::default_board;
use crate::web::web_state::WebStateHandle;

fn seed_board(h: &WebStateHandle, board_id: &str, project: &str) {
    h.db_for_test().insert_kanban_board(
        &default_board(board_id.into(), project.into()),
        "2026-01-01T00:00:00Z",
    );
}

async fn make_task(h: &WebStateHandle, board_id: &str, lane: &str) -> Task {
    h.create_kanban_task(CreateTaskRequest {
        board_id: board_id.into(),
        lane_id: lane.into(),
        title: "Task".into(),
        description: String::new(),
        tags: vec![],
        priority: "normal".into(),
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn internal_set_lane_not_found() {
    let h = WebStateHandle::new_test();
    assert!(matches!(
        h.kanban_internal_set_lane("ghost", "lane_planning", None)
            .await,
        Err(KanbanError::NotFound)
    ));
}

#[tokio::test]
async fn internal_set_lane_unknown_lane_forbidden() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    assert!(matches!(
        h.kanban_internal_set_lane(&t.id, "Nonexistent", None).await,
        Err(KanbanError::Forbidden(_))
    ));
}

#[tokio::test]
async fn internal_set_lane_illegal_transition_forbidden() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // todo → done is not an edge.
    assert!(matches!(
        h.kanban_internal_set_lane(&t.id, "lane_done", None).await,
        Err(KanbanError::Forbidden(_))
    ));
}

#[tokio::test]
async fn internal_set_lane_by_name_records_note_and_run_state() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    let moved = h
        .kanban_internal_set_lane(&t.id, "Planning", Some("running".into()))
        .await
        .unwrap();
    assert_eq!(moved.lane_id, "lane_planning");
    assert_eq!(moved.run_state, "running");
    // A move note was recorded.
    let notes = h.db_for_test().kanban_notes_for_task(&t.id);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].lane_to.as_deref(), Some("lane_planning"));
}

#[tokio::test]
async fn internal_set_lane_same_lane_no_note() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    let moved = h
        .kanban_internal_set_lane(&t.id, "lane_todo", None)
        .await
        .unwrap();
    assert_eq!(moved.lane_id, "lane_todo");
    // from == target → no transition note.
    assert!(h.db_for_test().kanban_notes_for_task(&t.id).is_empty());
}

#[tokio::test]
async fn internal_complete_moves_to_terminal_with_default_summary() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    let done = h.kanban_internal_complete(&t.id, "").await.unwrap();
    assert_eq!(done.lane_id, "lane_inreview"); // terminal lane
    assert_eq!(done.run_state, "done");
    let notes = h.db_for_test().kanban_notes_for_task(&t.id);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].body, "Completed — ready for review");
}

#[tokio::test]
async fn internal_complete_custom_summary_and_not_found() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    let done = h
        .kanban_internal_complete(&t.id, "all shipped")
        .await
        .unwrap();
    let notes = h.db_for_test().kanban_notes_for_task(&t.id);
    assert_eq!(notes[0].body, "all shipped");
    assert_eq!(done.lane_id, "lane_inreview");

    assert!(matches!(
        h.kanban_internal_complete("ghost", "x").await,
        Err(KanbanError::NotFound)
    ));
}

#[tokio::test]
async fn internal_complete_no_terminal_lane_stays_put() {
    let h = WebStateHandle::new_test();
    // Board with no terminal lane.
    let board = Board {
        id: "b2".into(),
        name: "B".into(),
        project_path: "/p2".into(),
        lanes: vec![Lane {
            id: "only".into(),
            name: "Only".into(),
            color: "#fff".into(),
            wip: None,
            terminal: false,
            agent: None,
            model: None,
            prompt: None,
        }],
        transitions: Default::default(),
    };
    h.db_for_test()
        .insert_kanban_board(&board, "2026-01-01T00:00:00Z");
    let t = make_task(&h, "b2", "only").await;
    let done = h.kanban_internal_complete(&t.id, "").await.unwrap();
    assert_eq!(done.lane_id, "only"); // no terminal → unchanged
    assert_eq!(done.run_state, "done");
}

#[tokio::test]
async fn internal_note_not_found_and_appends() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    assert!(matches!(
        h.kanban_internal_note("ghost", "b", None, None).await,
        Err(KanbanError::NotFound)
    ));
    let t = make_task(&h, "brd", "lane_todo").await;
    h.kanban_internal_note(&t.id, "note body", Some("a".into()), Some("b".into()))
        .await
        .unwrap();
    let notes = h.db_for_test().kanban_notes_for_task(&t.id);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].author, "agent");
    assert_eq!(notes[0].body, "note body");
}

#[tokio::test]
async fn add_user_note_not_found_and_idle_task() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    assert!(matches!(
        h.kanban_add_user_note("ghost", "hi").await,
        Err(KanbanError::NotFound)
    ));
    // Idle task (not running) → note recorded, no session forwarding.
    let t = make_task(&h, "brd", "lane_todo").await;
    let note = h.kanban_add_user_note(&t.id, "please hurry").await.unwrap();
    assert_eq!(note.author, "user");
    assert_eq!(note.body, "please hurry");
    assert_eq!(h.db_for_test().kanban_notes_for_task(&t.id).len(), 1);
}

#[tokio::test]
async fn add_user_note_live_task_takes_forward_branch() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // Make the task "live" so the forward-to-session branch is taken.
    h.set_kanban_task_launch(&t.id, Some("sess-live".into()), None, None, "running")
        .await
        .unwrap();
    let note = h
        .kanban_add_user_note(&t.id, "mid-flight note")
        .await
        .unwrap();
    assert_eq!(note.author, "user");
    // The note is still persisted regardless of the fire-and-forget forward.
    assert_eq!(h.db_for_test().kanban_notes_for_task(&t.id).len(), 1);
}
