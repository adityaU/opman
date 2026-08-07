//! Generated coverage tests for `db/routines.rs`: update-row, ordering, and
//! every trigger/target-mode string<->enum conversion.
use super::*;

fn base_routine(id: &str, updated: &str) -> RoutineDefinition {
    RoutineDefinition {
        id: id.into(),
        name: format!("r-{id}"),
        trigger: RoutineTrigger::Manual,
        enabled: false,
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
        updated_at: updated.into(),
    }
}

#[test]
fn list_sorted_by_updated_desc() {
    let db = Db::open_memory().unwrap();
    db.insert_routine(&base_routine("a", "2025-01-01T00:00:00Z"));
    db.insert_routine(&base_routine("b", "2025-03-01T00:00:00Z"));
    db.insert_routine(&base_routine("c", "2025-02-01T00:00:00Z"));
    let list = db.list_routines();
    assert_eq!(
        list.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        vec!["b", "c", "a"]
    );
}

#[test]
fn update_routine_row_found_and_not_found() {
    let db = Db::open_memory().unwrap();
    let mut r = base_routine("u1", "2025-01-01T00:00:00Z");
    db.insert_routine(&r);

    r.name = "renamed".into();
    r.enabled = true;
    r.trigger = RoutineTrigger::DailySummary;
    r.target_mode = Some(RoutineTargetMode::NewSession);
    r.project_index = Some(3);
    r.cron_expr = Some("* * * * *".into());
    r.updated_at = "2025-02-01T00:00:00Z".into();
    assert!(db.update_routine_row(&r));

    let got = &db.list_routines()[0];
    assert_eq!(got.name, "renamed");
    assert!(got.enabled);
    assert_eq!(got.trigger, RoutineTrigger::DailySummary);
    assert_eq!(got.target_mode, Some(RoutineTargetMode::NewSession));
    assert_eq!(got.project_index, Some(3));

    assert!(!db.update_routine_row(&base_routine("ghost", "2025-01-01T00:00:00Z")));
}

#[test]
fn delete_routine_row_missing_is_false() {
    let db = Db::open_memory().unwrap();
    assert!(!db.delete_routine_row("nope"));
}

#[test]
fn routine_run_ordering_desc() {
    let db = Db::open_memory().unwrap();
    for (id, created) in [
        ("r1", "2025-01-01"),
        ("r2", "2025-01-03"),
        ("r3", "2025-01-02"),
    ] {
        db.insert_routine_run(&RoutineRunRecord {
            id: id.into(),
            routine_id: "x".into(),
            status: "completed".into(),
            summary: String::new(),
            target_session_id: None,
            duration_ms: None,
            created_at: created.into(),
        });
    }
    let runs = db.list_routine_runs();
    assert_eq!(
        runs.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        vec!["r2", "r3", "r1"]
    );
}

#[test]
fn trigger_conversions_roundtrip_and_unknown() {
    for t in [
        RoutineTrigger::Manual,
        RoutineTrigger::Scheduled,
        RoutineTrigger::OnSessionIdle,
        RoutineTrigger::DailySummary,
    ] {
        assert_eq!(parse_trigger(trigger_str(&t)), t);
    }
    assert_eq!(parse_trigger("nonsense"), RoutineTrigger::Manual);
}

#[test]
fn target_mode_conversions_roundtrip_and_unknown() {
    for m in [
        RoutineTargetMode::ExistingSession,
        RoutineTargetMode::NewSession,
    ] {
        assert_eq!(parse_target_mode(target_mode_str(&m)), m);
    }
    assert_eq!(parse_target_mode("???"), RoutineTargetMode::ExistingSession);
}
