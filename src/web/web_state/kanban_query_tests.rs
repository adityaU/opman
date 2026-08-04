//! Generated coverage tests for `kanban_query.rs` (read-only board queries).
use super::KanbanError;
use super::*;
use crate::web::types::default_board;
use crate::web::web_state::WebStateHandle;

fn task(id: &str, board: &str, lane: &str, title: &str, tags: &[&str], archived: bool) -> Task {
    Task {
        id: id.into(),
        board_id: board.into(),
        lane_id: lane.into(),
        title: title.into(),
        description: format!("desc of {title}"),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        priority: "normal".into(),
        order_index: 1.0,
        session_id: None,
        launch_model: None,
        launch_agent: None,
        run_state: "idle".into(),
        archived,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn seed(handle: &WebStateHandle) {
    let db = handle.db_for_test();
    db.insert_kanban_board(
        &default_board("brd".into(), "/p".into()),
        "2026-01-01T00:00:00Z",
    );
    db.insert_kanban_task(&task(
        "anchor",
        "brd",
        "lane_todo",
        "Alpha",
        &["Backend", "urgent"],
        false,
    ));
    db.insert_kanban_task(&task(
        "t2",
        "brd",
        "lane_planning",
        "Beta frontend",
        &["frontend"],
        false,
    ));
    db.insert_kanban_task(&task("t3", "brd", "lane_todo", "Gamma", &[], true));
}

#[tokio::test]
async fn query_anchor_not_found() {
    let h = WebStateHandle::new_test();
    assert!(matches!(
        h.kanban_query_tasks("nope", None, &[], None, false).await,
        Err(KanbanError::NotFound)
    ));
}

#[tokio::test]
async fn query_excludes_archived_by_default_includes_when_asked() {
    let h = WebStateHandle::new_test();
    seed(&h);
    let (_, tasks) = h
        .kanban_query_tasks("anchor", None, &[], None, false)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 2); // t3 archived excluded
    let (_, all) = h
        .kanban_query_tasks("anchor", None, &[], None, true)
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
}

#[tokio::test]
async fn query_by_lane_id_and_name_and_unknown() {
    let h = WebStateHandle::new_test();
    seed(&h);
    let (_, by_id) = h
        .kanban_query_tasks("anchor", Some("lane_todo"), &[], None, false)
        .await
        .unwrap();
    assert_eq!(by_id.len(), 1);
    assert_eq!(by_id[0].id, "anchor");

    let (_, by_name) = h
        .kanban_query_tasks("anchor", Some("Planning"), &[], None, false)
        .await
        .unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].id, "t2");

    // Empty lane string is treated as "no filter".
    let (_, empty_lane) = h
        .kanban_query_tasks("anchor", Some(""), &[], None, false)
        .await
        .unwrap();
    assert_eq!(empty_lane.len(), 2);

    assert!(matches!(
        h.kanban_query_tasks("anchor", Some("Nonexistent"), &[], None, false)
            .await,
        Err(KanbanError::Forbidden(_))
    ));
}

#[tokio::test]
async fn query_by_tags_case_insensitive() {
    let h = WebStateHandle::new_test();
    seed(&h);
    let (_, tasks) = h
        .kanban_query_tasks("anchor", None, &["BACKEND".to_string()], None, false)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "anchor");
}

#[tokio::test]
async fn query_by_text_matches_title_desc_tags() {
    let h = WebStateHandle::new_test();
    seed(&h);
    // Matches title "Beta frontend" and its tag "frontend".
    let (_, by_text) = h
        .kanban_query_tasks("anchor", None, &[], Some("frontend"), false)
        .await
        .unwrap();
    assert_eq!(by_text.len(), 1);
    assert_eq!(by_text[0].id, "t2");

    // Matches description ("desc of Alpha").
    let (_, by_desc) = h
        .kanban_query_tasks("anchor", None, &[], Some("desc of alpha"), false)
        .await
        .unwrap();
    assert_eq!(by_desc.len(), 1);

    // Empty text string is ignored (no filter).
    let (_, empty) = h
        .kanban_query_tasks("anchor", None, &[], Some(""), false)
        .await
        .unwrap();
    assert_eq!(empty.len(), 2);
}

#[tokio::test]
async fn board_overview_returns_all_tasks() {
    let h = WebStateHandle::new_test();
    seed(&h);
    let (board, tasks) = h.kanban_board_overview("anchor").await.unwrap();
    assert_eq!(board.id, "brd");
    assert_eq!(tasks.len(), 3); // includes archived
    assert!(matches!(
        h.kanban_board_overview("ghost").await,
        Err(KanbanError::NotFound)
    ));
}

#[tokio::test]
async fn read_notes_defaults_to_anchor_and_skips_foreign() {
    let h = WebStateHandle::new_test();
    seed(&h);
    let db = h.db_for_test();
    db.insert_kanban_note(
        &KanbanNote {
            id: "n1".into(),
            author: "agent".into(),
            body: "hi".into(),
            lane_from: None,
            lane_to: None,
            created_at: "2026-01-01T00:00:01Z".into(),
        },
        "anchor",
    );
    // Second board + task, not on the anchor's board.
    db.insert_kanban_board(
        &default_board("brd2".into(), "/p2".into()),
        "2026-01-01T00:00:00Z",
    );
    db.insert_kanban_task(&task("foreign", "brd2", "lane_todo", "X", &[], false));

    // Empty ids → defaults to the anchor task.
    let default_out = h.kanban_read_notes("anchor", &[]).await.unwrap();
    assert_eq!(default_out.len(), 1);
    assert_eq!(default_out[0].0.id, "anchor");
    assert_eq!(default_out[0].1.len(), 1);

    // Explicit ids: unknown skipped, foreign-board skipped, valid kept.
    let out = h
        .kanban_read_notes(
            "anchor",
            &["anchor".into(), "unknown".into(), "foreign".into()],
        )
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0.id, "anchor");
}
