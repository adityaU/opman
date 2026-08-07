//! Coverage for `WebStateHandle::build_inner`'s persisted-collection loading
//! branches: seed a DB with one row of each kind, then build the inner state and
//! assert every collection's mapping closure ran (empty-db defaults are already
//! covered by `mod_tests.rs`).

use super::*;
use crate::web::db::Db;
use crate::web::types::*;
use std::path::PathBuf;

fn ts() -> String {
    "2025-01-01T00:00:00Z".into()
}

#[test]
fn build_inner_loads_all_persisted_collections() {
    let db = Db::open_memory().unwrap();

    db.insert_memory(&PersonalMemoryItem {
        id: "mem1".into(),
        label: "L".into(),
        content: "C".into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
        created_at: ts(),
        updated_at: ts(),
    });
    db.insert_routine(&RoutineDefinition {
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
        created_at: ts(),
        updated_at: ts(),
    });
    db.insert_routine_run(&RoutineRunRecord {
        id: "rr1".into(),
        routine_id: "r1".into(),
        status: "completed".into(),
        summary: "ok".into(),
        target_session_id: None,
        duration_ms: None,
        created_at: ts(),
    });

    let projects = vec![WebProject {
        name: "p".into(),
        path: PathBuf::from("/proj"),
        sessions: Vec::new(),
        active_session: None,
        git_branch: String::new(),
    }];

    let inner = WebStateHandle::build_inner(&db, projects);

    // Every collection's mapping closure keyed the row by its id/name.
    assert!(inner.personal_memory.contains_key("mem1"));
    assert!(inner.routines.contains_key("r1"));
    assert_eq!(inner.routine_runs.len(), 1);
    // Projects carried through untouched.
    assert_eq!(inner.projects.len(), 1);
    assert_eq!(inner.active_project, 0);
}
