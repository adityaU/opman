use super::*;
use crate::web::db::Db;

fn memory_item() -> PersonalMemoryItem {
    PersonalMemoryItem {
        id: "mem1".into(),
        label: "lbl".into(),
        content: "note".into(),
        scope: MemoryScope::Project,
        project_index: Some(1),
        session_id: Some("s1".into()),
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn routine() -> RoutineDefinition {
    RoutineDefinition {
        id: "r1".into(),
        name: "daily".into(),
        trigger: RoutineTrigger::Scheduled,
        enabled: true,
        cron_expr: Some("0 9 * * *".into()),
        timezone: Some("UTC".into()),
        target_mode: Some(RoutineTargetMode::ExistingSession),
        session_id: Some("s1".into()),
        project_index: Some(0),
        prompt: Some("hi".into()),
        provider_id: Some("anthropic".into()),
        model_id: Some("claude".into()),
        last_run_at: Some("t".into()),
        next_run_at: Some("t2".into()),
        last_error: None,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn run_record() -> RoutineRunRecord {
    RoutineRunRecord {
        id: "run1".into(),
        routine_id: "r1".into(),
        status: "completed".into(),
        summary: "ok".into(),
        target_session_id: Some("s1".into()),
        duration_ms: Some(123),
        created_at: "t".into(),
    }
}

fn autonomy() -> AutonomySettings {
    AutonomySettings {
        mode: AutonomyMode::Nudge,
        updated_at: "t".into(),
    }
}

#[test]
fn sync_all_round_trip() {
    let db = Db::open_memory().expect("mem db");
    let memory = vec![memory_item()];
    let auto = autonomy();
    let routines = vec![routine()];
    let runs = vec![run_record()];

    super::sync_all(&db, &memory, &auto, &routines, &runs).expect("sync ok");

    assert_eq!(db.list_memory().len(), 1);
    assert_eq!(db.list_memory()[0].content, "note");
    assert_eq!(db.list_routines().len(), 1);
    assert_eq!(db.list_routines()[0].name, "daily");
    assert_eq!(db.list_routine_runs().len(), 1);
    assert_eq!(db.list_routine_runs()[0].summary, "ok");
    assert!(matches!(
        db.load_autonomy_settings().mode,
        AutonomyMode::Nudge
    ));
}

#[test]
fn sync_all_replaces_previous_rows() {
    let db = Db::open_memory().expect("mem db");
    let auto = autonomy();
    // First write with one memory item and one routine.
    super::sync_all(&db, &[memory_item()], &auto, &[routine()], &[run_record()]).unwrap();
    assert_eq!(db.list_memory().len(), 1);
    assert_eq!(db.list_routines().len(), 1);
    assert_eq!(db.list_routine_runs().len(), 1);
    // Second write with empty slices clears everything (DELETE then re-insert).
    super::sync_all(&db, &[], &auto, &[], &[]).unwrap();
    assert_eq!(db.list_memory().len(), 0);
    assert_eq!(db.list_routines().len(), 0);
    assert_eq!(db.list_routine_runs().len(), 0);
}

#[test]
fn sync_all_empty_is_ok() {
    let db = Db::open_memory().expect("mem db");
    let auto = autonomy();
    super::sync_all(&db, &[], &auto, &[], &[]).expect("empty ok");
    assert_eq!(db.list_memory().len(), 0);
    assert_eq!(db.list_routines().len(), 0);
}

// ── String conversion helpers: every variant ────────────────────────

#[test]
fn scope_str_all_variants() {
    use super::scope_str;
    assert_eq!(scope_str(&MemoryScope::Global), "global");
    assert_eq!(scope_str(&MemoryScope::Project), "project");
    assert_eq!(scope_str(&MemoryScope::Session), "session");
}

#[test]
fn mode_str_all_variants() {
    use super::mode_str;
    assert_eq!(mode_str(&AutonomyMode::Observe), "observe");
    assert_eq!(mode_str(&AutonomyMode::Nudge), "nudge");
    assert_eq!(mode_str(&AutonomyMode::Continue), "continue");
    assert_eq!(mode_str(&AutonomyMode::Autonomous), "autonomous");
}

#[test]
fn trigger_str_all_variants() {
    use super::trigger_str;
    assert_eq!(trigger_str(&RoutineTrigger::Manual), "manual");
    assert_eq!(trigger_str(&RoutineTrigger::Scheduled), "scheduled");
    assert_eq!(
        trigger_str(&RoutineTrigger::OnSessionIdle),
        "on_session_idle"
    );
    assert_eq!(trigger_str(&RoutineTrigger::DailySummary), "daily_summary");
}

#[test]
fn target_mode_str_all_variants() {
    use super::target_mode_str;
    assert_eq!(
        target_mode_str(&RoutineTargetMode::ExistingSession),
        "existing_session"
    );
    assert_eq!(
        target_mode_str(&RoutineTargetMode::NewSession),
        "new_session"
    );
}
