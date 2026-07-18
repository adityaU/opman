//! Generated coverage tests for `schema.rs` (DDL: create_tables / create_indexes).
use super::*;
use rusqlite::Connection;

fn table_names(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();
    let rows: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    rows
}

#[test]
fn create_tables_creates_every_expected_table() {
    let conn = Connection::open_in_memory().unwrap();
    create_tables(&conn).unwrap();
    let names = table_names(&conn);
    for expected in [
        "missions",
        "personal_memory",
        "autonomy_settings",
        "routines",
        "routine_runs",
        "delegated_work",
        "workspaces",
        "signals",
        "kanban_boards",
        "kanban_tasks",
        "kanban_attachments",
        "kanban_notes",
        "kanban_pipeline_runs",
    ] {
        assert!(names.iter().any(|n| n == expected), "missing table {expected}");
    }
}

#[test]
fn create_tables_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    create_tables(&conn).unwrap();
    // Second call must not error (IF NOT EXISTS).
    create_tables(&conn).unwrap();
}

#[test]
fn create_indexes_succeeds_and_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    create_tables(&conn).unwrap();
    create_indexes(&conn).unwrap();
    create_indexes(&conn).unwrap();

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index'")
        .unwrap();
    let idx: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(idx.iter().any(|n| n == "idx_kanban_tasks_board"));
    assert!(idx.iter().any(|n| n == "idx_signals_created"));
}
