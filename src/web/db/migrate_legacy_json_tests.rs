//! Coverage tests for the legacy JSON→SQLite import.
use super::*;

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
        enabled: true,
        cron_expr: None,
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
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
    serde_json::json!({
        "personal_memory": { "m1": serde_json::to_value(&mem).unwrap() },
        "autonomy_settings": serde_json::to_value(&settings).unwrap(),
        "routines": { "r1": serde_json::to_value(&routine).unwrap() },
        "routine_runs": [serde_json::to_value(&run).unwrap()],
    })
}

fn unique_tmp(name: &str) -> PathBuf {
    let n: u64 = rand::random();
    std::env::temp_dir().join(format!("opman_migtest_{name}_{n}.json"))
}

#[test]
fn run_migration_from_imports_all_records_and_renames_file() {
    let db = Db::open_memory().unwrap();
    let path = unique_tmp("import");
    std::fs::write(&path, serde_json::to_string(&legacy_json_value()).unwrap()).unwrap();

    run_migration_from(&db, path.clone());

    assert_eq!(db.list_memory().len(), 1);
    assert!(matches!(
        db.load_autonomy_settings().mode,
        AutonomyMode::Continue
    ));
    assert_eq!(db.list_routines().len(), 1);
    assert_eq!(db.list_routine_runs().len(), 1);

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
fn run_migration_with_no_json_file_is_a_noop() {
    let db = Db::open_memory().unwrap();
    run_migration(&db);
    assert!(db.list_memory().is_empty());
}
