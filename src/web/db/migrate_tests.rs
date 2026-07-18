//! Generated coverage tests for `migrate.rs`:
//! JSON import (`run_migration_from`), routines v1→v2, kanban `archived` column.
use super::*;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

fn db_from_raw(conn: Connection) -> Db {
    Db {
        conn: Arc::new(Mutex::new(conn)),
    }
}

fn unique_tmp(name: &str) -> PathBuf {
    let n: u64 = rand::random();
    std::env::temp_dir().join(format!("opman_migtest_{name}_{n}.json"))
}

fn sample_workspace() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        name: "coding".into(),
        created_at: "2025-01-01T00:00:00Z".into(),
        panels: WorkspacePanels {
            sidebar: true,
            terminal: false,
            editor: true,
            git: false,
        },
        layout: WorkspaceLayout::default(),
        open_files: vec!["main.rs".into()],
        active_file: None,
        terminal_tabs: vec![],
        session_id: None,
        git_branch: None,
        is_template: false,
        recipe_description: None,
        recipe_next_action: None,
        is_recipe: false,
    }
}

fn legacy_json_value() -> serde_json::Value {
    let mem = PersonalMemoryItem {
        id: "m1".into(),
        label: "L".into(),
        content: "C".into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
    };
    let settings = AutonomySettings {
        mode: AutonomyMode::Continue,
        updated_at: "2025-01-01T00:00:00Z".into(),
    };
    let routine = RoutineDefinition {
        id: "r1".into(),
        name: "R".into(),
        trigger: RoutineTrigger::Manual,
        action: RoutineAction::SendMessage,
        enabled: true,
        cron_expr: None,
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
        mission_id: None,
        last_run_at: None,
        next_run_at: None,
        last_error: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
    };
    let run = RoutineRunRecord {
        id: "rr1".into(),
        routine_id: "r1".into(),
        status: "completed".into(),
        summary: "ok".into(),
        target_session_id: None,
        duration_ms: None,
        created_at: "2025-01-01T00:00:00Z".into(),
    };
    let dw = DelegatedWorkItem {
        id: "d1".into(),
        title: "T".into(),
        assignee: "a".into(),
        scope: "s".into(),
        status: DelegationStatus::Planned,
        mission_id: None,
        session_id: None,
        subagent_session_id: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
    };
    let sig = SignalInput {
        id: "s1".into(),
        kind: "k".into(),
        title: "ttl".into(),
        body: "b".into(),
        created_at: 1.0,
        session_id: None,
    };
    serde_json::json!({
        "personal_memory": { "m1": serde_json::to_value(&mem).unwrap() },
        "autonomy_settings": serde_json::to_value(&settings).unwrap(),
        "routines": { "r1": serde_json::to_value(&routine).unwrap() },
        "routine_runs": [serde_json::to_value(&run).unwrap()],
        "delegated_work": { "d1": serde_json::to_value(&dw).unwrap() },
        "workspaces": { "coding": serde_json::to_value(&sample_workspace()).unwrap() },
        "signals": [serde_json::to_value(&sig).unwrap()],
    })
}

#[test]
fn run_migration_from_imports_all_records_and_renames_file() {
    let db = Db::open_memory().unwrap();
    let path = unique_tmp("import");
    std::fs::write(&path, serde_json::to_string(&legacy_json_value()).unwrap()).unwrap();

    run_migration_from(&db, path.clone());

    assert_eq!(db.list_memory().len(), 1);
    assert!(matches!(db.load_autonomy_settings().mode, AutonomyMode::Continue));
    assert_eq!(db.list_routines().len(), 1);
    assert_eq!(db.list_routine_runs().len(), 1);
    assert_eq!(db.list_delegated_work().len(), 1);
    assert_eq!(db.list_workspaces().len(), 1);
    assert_eq!(db.list_signals(100).len(), 1);
    // Missions are intentionally NOT imported.
    assert!(db.list_missions().is_empty());

    // Original renamed to .bak so a re-run is a no-op.
    assert!(!path.exists());
    let bak = path.with_extension("json.bak");
    assert!(bak.exists());
    let _ = std::fs::remove_file(&bak);
}

#[test]
fn run_migration_from_no_file_is_noop() {
    let db = Db::open_memory().unwrap();
    let path = unique_tmp("missing");
    // File does not exist → early return after schema migrations.
    run_migration_from(&db, path);
    assert!(db.list_memory().is_empty());
}

#[test]
fn run_migration_from_skips_when_db_has_data() {
    let db = Db::open_memory().unwrap();
    db.insert_memory(&PersonalMemoryItem {
        id: "pre".into(),
        label: "pre".into(),
        content: "".into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
    });
    let path = unique_tmp("hasdata");
    std::fs::write(&path, serde_json::to_string(&legacy_json_value()).unwrap()).unwrap();

    run_migration_from(&db, path.clone());

    // Only the pre-existing item; import skipped. File not renamed.
    assert_eq!(db.list_memory().len(), 1);
    assert!(db.list_routines().is_empty());
    assert!(path.exists());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn run_migration_from_bad_json_returns() {
    let db = Db::open_memory().unwrap();
    let path = unique_tmp("badjson");
    std::fs::write(&path, "{ this is not valid json ").unwrap();

    run_migration_from(&db, path.clone());

    assert!(db.list_memory().is_empty());
    // Parse failed → file left in place (not renamed).
    assert!(path.exists());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn schema_migrations_idempotent_on_fresh_db() {
    // open_memory already ran migrations; running again exercises the
    // early-return branches of every migrator (state / enabled / archived exist).
    let db = Db::open_memory().unwrap();
    run_schema_migrations(&db);
    run_schema_migrations(&db);
    let conn = db.conn();
    assert!(conn.prepare("SELECT archived FROM kanban_tasks LIMIT 0").is_ok());
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

    // Create the remaining tables (missions/etc.) so run_schema_migrations works.
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
    assert_eq!(routines[0].action, RoutineAction::ReviewMission);
    assert_eq!(routines[0].mission_id.as_deref(), Some("mi1"));

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
    assert!(conn.prepare("SELECT archived FROM kanban_tasks LIMIT 0").is_err());

    // Remaining tables (missions already has `state`, routines has `enabled`).
    super::super::schema::create_tables(&conn).unwrap();

    let db = db_from_raw(conn);
    run_schema_migrations(&db);

    let conn = db.conn();
    assert!(conn.prepare("SELECT archived FROM kanban_tasks LIMIT 0").is_ok());
}
