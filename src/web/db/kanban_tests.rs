//! Generated coverage tests for `db/kanban.rs` — boards, tasks, attachments, notes.
use super::*;
use crate::web::types::default_board;

fn mk_task(id: &str, board: &str, lane: &str, order: f64) -> Task {
    Task {
        id: id.into(),
        board_id: board.into(),
        lane_id: lane.into(),
        title: format!("t-{id}"),
        description: String::new(),
        tags: vec![],
        priority: "normal".into(),
        order_index: order,
        session_id: None,
        launch_model: None,
        launch_agent: None,
        run_state: "idle".into(),
        archived: false,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[test]
fn board_lookup_by_id_and_project_and_missing() {
    let db = Db::open_memory().unwrap();
    let board = default_board("brd_x".into(), "/p".into());
    db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");

    assert!(db.kanban_board("brd_x").is_some());
    assert!(db.kanban_board("nope").is_none());
    assert!(db.kanban_board_for_project("/p").is_some());
    assert!(db.kanban_board_for_project("/missing").is_none());
}

#[test]
fn update_board_config_found_and_not_found() {
    let db = Db::open_memory().unwrap();
    let mut board = default_board("brd_c".into(), "/pc".into());
    db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");

    board.name = "Renamed".into();
    board.lanes.truncate(2);
    assert!(db.update_kanban_board_config(&board, "2026-01-02T00:00:00Z"));
    let fetched = db.kanban_board("brd_c").unwrap();
    assert_eq!(fetched.name, "Renamed");
    assert_eq!(fetched.lanes.len(), 2);

    // Unknown id → no rows changed → false.
    let ghost = default_board("ghost".into(), "/g".into());
    assert!(!db.update_kanban_board_config(&ghost, "2026-01-02T00:00:00Z"));
}

#[test]
fn max_order_and_task_crud() {
    let db = Db::open_memory().unwrap();
    let board = default_board("brd_o".into(), "/po".into());
    db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");

    // Empty lane → COALESCE(MAX,0) == 0.0
    assert_eq!(db.kanban_max_order("brd_o", "lane_todo"), 0.0);

    db.insert_kanban_task(&mk_task("t1", "brd_o", "lane_todo", 1.0));
    db.insert_kanban_task(&mk_task("t2", "brd_o", "lane_todo", 5.0));
    assert_eq!(db.kanban_max_order("brd_o", "lane_todo"), 5.0);

    // kanban_task by id, present + absent.
    assert!(db.kanban_task("t1").is_some());
    assert!(db.kanban_task("absent").is_none());

    // Ordered by lane then order_index.
    let tasks = db.kanban_tasks_for_board("brd_o");
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "t1");

    // update_kanban_task: found true, unknown false.
    let mut t = db.kanban_task("t1").unwrap();
    t.title = "changed".into();
    t.archived = true;
    t.priority = "high".into();
    assert!(db.update_kanban_task(&t));
    let reread = db.kanban_task("t1").unwrap();
    assert_eq!(reread.title, "changed");
    assert!(reread.archived);
    assert_eq!(reread.priority, "high");

    let ghost = mk_task("ghost", "brd_o", "lane_todo", 0.0);
    assert!(!db.update_kanban_task(&ghost));

    assert!(db.delete_kanban_task("t1"));
    assert!(!db.delete_kanban_task("t1"));
}

#[test]
fn attachments_insert_and_list_ordered() {
    let db = Db::open_memory().unwrap();
    let board = default_board("brd_a".into(), "/pa".into());
    db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");
    db.insert_kanban_task(&mk_task("ta", "brd_a", "lane_todo", 1.0));

    assert!(db.kanban_attachments_for_task("ta").is_empty());

    db.insert_kanban_attachment(&Attachment {
        id: "att1".into(),
        task_id: "ta".into(),
        filename: "a.png".into(),
        mime: "image/png".into(),
        kind: "image".into(),
        size_bytes: 10,
        created_at: "2026-01-01T00:00:01Z".into(),
        url: String::new(),
    });
    db.insert_kanban_attachment(&Attachment {
        id: "att2".into(),
        task_id: "ta".into(),
        filename: "b.txt".into(),
        mime: "text/plain".into(),
        kind: "file".into(),
        size_bytes: 20,
        created_at: "2026-01-01T00:00:02Z".into(),
        url: String::new(),
    });
    let list = db.kanban_attachments_for_task("ta");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "att1");
    assert_eq!(list[1].filename, "b.txt");
    assert_eq!(list[0].url, "");
}

#[test]
fn notes_insert_and_list_ordered() {
    let db = Db::open_memory().unwrap();
    let board = default_board("brd_n".into(), "/pn".into());
    db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");
    db.insert_kanban_task(&mk_task("tn", "brd_n", "lane_todo", 1.0));

    assert!(db.kanban_notes_for_task("tn").is_empty());

    db.insert_kanban_note(
        &KanbanNote {
            id: "n1".into(),
            author: "agent".into(),
            body: "first".into(),
            lane_from: None,
            lane_to: None,
            created_at: "2026-01-01T00:00:01Z".into(),
        },
        "tn",
    );
    db.insert_kanban_note(
        &KanbanNote {
            id: "n2".into(),
            author: "user".into(),
            body: "second".into(),
            lane_from: Some("lane_todo".into()),
            lane_to: Some("lane_planning".into()),
            created_at: "2026-01-01T00:00:02Z".into(),
        },
        "tn",
    );
    let notes = db.kanban_notes_for_task("tn");
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "n1");
    assert_eq!(notes[1].lane_to.as_deref(), Some("lane_planning"));
}

#[test]
fn tasks_for_board_unknown_board_is_empty() {
    let db = Db::open_memory().unwrap();
    assert!(db.kanban_tasks_for_board("no_board").is_empty());
}
