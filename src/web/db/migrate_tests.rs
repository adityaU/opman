//! Coverage tests for `migrate.rs`: routines v1→v2 and the kanban `archived` column.
use super::*;
use crate::web::types::RoutineTrigger;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn db_from_raw(conn: Connection) -> Db {
    Db {
        conn: Arc::new(Mutex::new(conn)),
    }
}

#[test]
fn schema_migrations_idempotent_on_fresh_db() {
    // open_memory already ran migrations; running again exercises the
    // early-return branches of every migrator (state / enabled / archived exist).
    let db = Db::open_memory().unwrap();
    run_schema_migrations(&db);
    run_schema_migrations(&db);
    let conn = db.conn();
    assert!(conn
        .prepare("SELECT archived FROM kanban_tasks LIMIT 0")
        .is_ok());
}

#[test]
fn migrate_routines_v1_to_v2_preserves_rows() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

    // v1 routines + routine_runs schema (no `enabled` column).
    conn.execute_batch(
        "CREATE TABLE routines (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, trigger TEXT NOT NULL,
            action TEXT NOT NULL, mission_id TEXT, session_id TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
         );
         CREATE TABLE routine_runs (
            id TEXT PRIMARY KEY, routine_id TEXT NOT NULL, status TEXT NOT NULL,
            summary TEXT NOT NULL, created_at TEXT NOT NULL
         );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO routines (id, name, trigger, action, mission_id, session_id, created_at, updated_at)
         VALUES ('r1','Nightly','scheduled','review_mission','mi1','se1','2025-01-01','2025-01-02')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO routine_runs (id, routine_id, status, summary, created_at)
         VALUES ('run1','r1','completed','done','2025-01-03')",
        [],
    )
    .unwrap();

    // Create the remaining tables so run_schema_migrations works.
    super::super::schema::create_tables(&conn).unwrap();

    let db = db_from_raw(conn);
    run_schema_migrations(&db);
    {
        let conn = db.conn();
        super::super::schema::create_indexes(&conn).unwrap();
    }

    let routines = db.list_routines();
    assert_eq!(routines.len(), 1);
    assert_eq!(routines[0].id, "r1");
    assert_eq!(routines[0].name, "Nightly");
    assert!(routines[0].enabled);
    assert_eq!(routines[0].trigger, RoutineTrigger::Scheduled);
    // The legacy `action` and `mission_id` columns are dropped, not carried over.
    assert_eq!(routines[0].session_id.as_deref(), Some("se1"));

    let runs = db.list_routine_runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "run1");
    assert_eq!(runs[0].summary, "done");
}

#[test]
fn migrate_adds_archived_column_to_old_kanban_tasks() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

    // Old kanban_tasks without the `archived` column.
    conn.execute_batch(
        "CREATE TABLE kanban_boards (
            id TEXT PRIMARY KEY, project_path TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
            lanes TEXT NOT NULL DEFAULT '[]', transitions TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
         );
         CREATE TABLE kanban_tasks (
            id TEXT PRIMARY KEY, board_id TEXT NOT NULL, lane_id TEXT NOT NULL,
            title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '[]', priority TEXT NOT NULL DEFAULT 'normal',
            order_index REAL NOT NULL DEFAULT 0, session_id TEXT, launch_model TEXT,
            launch_agent TEXT, run_state TEXT NOT NULL DEFAULT 'idle',
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
         );",
    )
    .unwrap();

    // Column is absent before migration.
    assert!(conn
        .prepare("SELECT archived FROM kanban_tasks LIMIT 0")
        .is_err());

    // Remaining tables (routines already has `enabled`).
    super::super::schema::create_tables(&conn).unwrap();

    let db = db_from_raw(conn);
    run_schema_migrations(&db);

    let conn = db.conn();
    assert!(conn
        .prepare("SELECT archived FROM kanban_tasks LIMIT 0")
        .is_ok());
}

/// One-off guard (ignored by default): run the real open+migrate path against a
/// copy of a production database and confirm the removed-feature tables are gone
/// while every surviving table keeps its rows.
/// Run with: OPMAN_MIG_DB=/path/to/copy.db cargo test migrate_real_db -- --ignored --nocapture
#[test]
#[ignore]
fn migrate_real_db_drops_dead_tables_and_keeps_survivors() {
    let Ok(path) = std::env::var("OPMAN_MIG_DB") else {
        return;
    };
    let db = Db::open_at(PathBuf::from(&path)).expect("open real db copy");
    run_schema_migrations(&db);
    run_schema_migrations(&db);

    let conn = db.conn();
    for dead in ["missions", "delegated_work", "workspaces", "signals"] {
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [dead],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "{dead} should be dropped");
    }
    drop(conn);

    println!(
        "memory={} routines={} runs={}",
        db.list_memory().len(),
        db.list_routines().len(),
        db.list_routine_runs().len()
    );
    assert_eq!(db.list_memory().len(), 10, "memory rows must survive");
    assert_eq!(db.list_routines().len(), 1, "routine rows must survive");
    assert_eq!(db.list_routine_runs().len(), 2, "run rows must survive");
}
