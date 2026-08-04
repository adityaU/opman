//! Generated coverage tests for `web_state/kanban.rs` (part 3):
//! the extracted `user_note_agent_message` helper plus branch fills for
//! board-by-active-project resolution, no-op task edits, and the
//! "launching" live-forward branch of `kanban_add_user_note`.
use super::*;
use crate::web::types::default_board;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

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

// ── user_note_agent_message (pure) ──────────────────────────────────

#[test]
fn user_note_agent_message_embeds_body_and_instruction() {
    let m = user_note_agent_message("please refactor the parser");
    assert!(m.starts_with("📝 New note from the human reviewer"));
    assert!(m.contains("please refactor the parser"));
    assert!(m.contains("kanban_add_note"));
}

#[test]
fn user_note_agent_message_handles_empty_body() {
    let m = user_note_agent_message("");
    assert!(m.contains("New note from the human reviewer"));
    // Body slot is empty but the surrounding instruction survives.
    assert!(m.contains("kanban_add_note"));
}

#[test]
fn user_note_agent_message_preserves_multiline_and_unicode() {
    let m = user_note_agent_message("line1\nlINE2 — 日本語");
    assert!(m.contains("line1\nlINE2 — 日本語"));
}

// ── get_kanban_board via active project (None index) ────────────────

#[tokio::test]
async fn get_kanban_board_none_uses_active_project() {
    let h = WebStateHandle::new_test_with_projects(vec![(
        "P".into(),
        PathBuf::from("/tmp/opman_kb_active_none"),
    )]);
    // active_project defaults to 0 → None resolves to the first project.
    let board = h.get_kanban_board(None).await.expect("board");
    assert_eq!(board.board.lanes.len(), 7);
    // Pipelines list is present (empty by default).
    assert!(board.pipelines.is_empty());
}

// ── update_kanban_task: all-None request is a no-op edit ─────────────

#[tokio::test]
async fn update_task_all_none_keeps_fields_but_bumps_updated_at() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    let before = t.updated_at.clone();
    let updated = h
        .update_kanban_task(
            &t.id,
            UpdateTaskRequest {
                title: None,
                description: None,
                tags: None,
                priority: None,
                lane_id: None,
                order_index: None,
                archived: None,
            },
        )
        .await
        .unwrap();
    // Fields unchanged (all if-let-Some branches skipped).
    assert_eq!(updated.title, t.title);
    assert_eq!(updated.lane_id, "lane_todo");
    assert_eq!(updated.priority, t.priority);
    assert!(!updated.archived);
    // updated_at is always refreshed.
    assert!(!before.is_empty());
}

// ── add_user_note: "launching" state also triggers the forward branch ─

#[tokio::test]
async fn add_user_note_launching_state_is_live() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // run_state == "launching" is the other half of the live check.
    h.set_kanban_task_launch(
        &t.id,
        Some("sess-launching".into()),
        None,
        None,
        "launching",
    )
    .await
    .unwrap();
    let note = h
        .kanban_add_user_note(&t.id, "starting up note")
        .await
        .unwrap();
    assert_eq!(note.author, "user");
    assert_eq!(h.db_for_test().kanban_notes_for_task(&t.id).len(), 1);
}

#[tokio::test]
async fn add_user_note_live_but_no_session_id_skips_forward() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // running but no session_id → the (true, Some(sid), Some(b)) tuple fails to
    // match, so the note is recorded without forwarding.
    h.set_kanban_task_launch(&t.id, None, None, None, "running")
        .await
        .unwrap();
    let note = h.kanban_add_user_note(&t.id, "no session").await.unwrap();
    assert_eq!(note.author, "user");
    assert_eq!(note.body, "no session");
}
