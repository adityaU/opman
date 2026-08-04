//! Generated coverage tests for `db/mod.rs` (Db::open_at, db_path, conn).
use super::*;
use crate::web::types::*;

fn unique_tmp(name: &str) -> PathBuf {
    let n: u64 = rand::random();
    std::env::temp_dir().join(format!("opman_dbtest_{name}_{n}"))
}

#[test]
fn open_at_creates_parent_and_schema() {
    // Nested parent dir that does not exist yet exercises create_dir_all.
    let dir = unique_tmp("open_at");
    let path = dir.join("nested").join("assistant.db");
    let db = Db::open_at(path.clone()).unwrap();

    // Schema is present + usable: write and read a row through a public method.
    db.save_autonomy_settings(&AutonomySettings {
        mode: AutonomyMode::Nudge,
        updated_at: "2025-01-01T00:00:00Z".into(),
    });
    assert!(matches!(
        db.load_autonomy_settings().mode,
        AutonomyMode::Nudge
    ));

    // conn() hands out a live connection guard.
    {
        let conn = db.conn();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(n >= 7);
    }

    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_at_reopen_existing_file() {
    let dir = unique_tmp("reopen");
    let path = dir.join("assistant.db");
    {
        let db = Db::open_at(path.clone()).unwrap();
        db.insert_memory(&PersonalMemoryItem {
            id: "m1".into(),
            label: "keep".into(),
            content: "x".into(),
            scope: MemoryScope::Global,
            project_index: None,
            session_id: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        });
    }
    // Reopening the same file path runs create_tables/migrations again (idempotent)
    // and preserves data.
    let db2 = Db::open_at(path).unwrap();
    assert_eq!(db2.list_memory().len(), 1);

    drop(db2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn db_path_points_at_opman_assistant_db() {
    let p = db_path();
    assert!(p.ends_with("opman/assistant.db"));
}
