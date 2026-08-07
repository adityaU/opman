use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

fn routine(
    id: &str,
    trigger: RoutineTrigger,
    cron: Option<&str>,
    enabled: bool,
    next_run: Option<&str>,
) -> RoutineDefinition {
    RoutineDefinition {
        id: id.into(),
        name: id.into(),
        trigger,
        enabled,
        cron_expr: cron.map(|s| s.to_string()),
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: Some(0),
        prompt: None,
        provider_id: None,
        model_id: None,
        last_run_at: None,
        next_run_at: next_run.map(|s| s.to_string()),
        last_error: None,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

async fn insert(h: &WebStateHandle, r: RoutineDefinition) {
    h.inner.write().await.routines.insert(r.id.clone(), r);
}

async fn next_run_of(h: &WebStateHandle, id: &str) -> Option<String> {
    h.inner
        .read()
        .await
        .routines
        .get(id)
        .and_then(|r| r.next_run_at.clone())
}

// ── compute_next_run ────────────────────────────────────────────────

#[test]
fn compute_next_run_field_arities() {
    assert!(super::compute_next_run("0 9 * * *", None).is_some()); // 5-field
    assert!(super::compute_next_run("0 0 9 * * *", None).is_some()); // 6-field
    assert!(super::compute_next_run("0 0 9 * * * *", None).is_some()); // 7-field
    assert!(super::compute_next_run("a b", None).is_none()); // bad arity
    assert!(super::compute_next_run("bad bad bad bad bad", None).is_none()); // invalid expr
}

#[test]
fn compute_next_run_timezone_handling() {
    // Valid IANA timezone path.
    assert!(super::compute_next_run("0 9 * * *", Some("America/New_York")).is_some());
    // Invalid timezone falls back to UTC computation.
    assert!(super::compute_next_run("0 9 * * *", Some("Not/AZone")).is_some());
}

// ── update_next_run ─────────────────────────────────────────────────

#[tokio::test]
async fn update_next_run_missing_routine_noop() {
    let h = WebStateHandle::new_test();
    h.update_next_run("ghost").await; // early return, no panic
}

#[tokio::test]
async fn update_next_run_no_cron_sets_none() {
    let h = WebStateHandle::new_test();
    insert(
        &h,
        routine(
            "r1",
            RoutineTrigger::Scheduled,
            None,
            true,
            Some("x"),
        ),
    )
    .await;
    h.update_next_run("r1").await;
    assert!(next_run_of(&h, "r1").await.is_none());
}

#[tokio::test]
async fn update_next_run_valid_cron_sets_some() {
    let h = WebStateHandle::new_test();
    insert(
        &h,
        routine(
            "r1",
            RoutineTrigger::Scheduled,
            Some("0 9 * * *"),
            true,
            None,
        ),
    )
    .await;
    h.update_next_run("r1").await;
    assert!(next_run_of(&h, "r1").await.is_some());
}

#[tokio::test]
async fn update_next_run_invalid_cron_sets_none() {
    let h = WebStateHandle::new_test();
    insert(
        &h,
        routine(
            "r1",
            RoutineTrigger::Scheduled,
            Some("nonsense"),
            true,
            Some("x"),
        ),
    )
    .await;
    h.update_next_run("r1").await;
    assert!(next_run_of(&h, "r1").await.is_none());
}

// ── recompute_all_next_runs / recompute_next_run_if_scheduled ───────

#[tokio::test]
async fn recompute_all_next_runs_updates_scheduled() {
    let h = WebStateHandle::new_test();
    insert(
        &h,
        routine(
            "sched",
            RoutineTrigger::Scheduled,
            Some("0 9 * * *"),
            true,
            None,
        ),
    )
    .await;
    insert(
        &h,
        routine(
            "manual",
            RoutineTrigger::Manual,
            None,
            true,
            None,
        ),
    )
    .await;
    h.recompute_all_next_runs().await;
    assert!(next_run_of(&h, "sched").await.is_some());
    assert!(next_run_of(&h, "manual").await.is_none());
}

#[tokio::test]
async fn recompute_next_run_if_scheduled_variants() {
    let h = WebStateHandle::new_test();
    // Not scheduled → no-op.
    insert(
        &h,
        routine(
            "manual",
            RoutineTrigger::Manual,
            Some("0 9 * * *"),
            true,
            None,
        ),
    )
    .await;
    h.recompute_next_run_if_scheduled("manual").await;
    assert!(next_run_of(&h, "manual").await.is_none());
    // Missing routine → no-op.
    h.recompute_next_run_if_scheduled("ghost").await;
    // Scheduled + enabled + cron → updates.
    insert(
        &h,
        routine(
            "sched",
            RoutineTrigger::Scheduled,
            Some("0 9 * * *"),
            true,
            None,
        ),
    )
    .await;
    h.recompute_next_run_if_scheduled("sched").await;
    assert!(next_run_of(&h, "sched").await.is_some());
}

// ── tick_scheduler ──────────────────────────────────────────────────

#[tokio::test]
async fn tick_scheduler_no_routines_is_noop() {
    let h = WebStateHandle::new_test();
    h.tick_scheduler().await;
}

#[tokio::test]
async fn tick_scheduler_fires_due_routine_and_skips_others() {
    let h = WebStateHandle::new_test();
    // Due: scheduled, enabled, cron set, next_run_at None. No prompt → execute_routine
    // fails fast without touching the network, but still records a run.
    insert(
        &h,
        routine(
            "due",
            RoutineTrigger::Scheduled,
            Some("0 9 * * *"),
            true,
            None,
        ),
    )
    .await;
    // Not due: next_run far in the future.
    insert(
        &h,
        routine(
            "future",
            RoutineTrigger::Scheduled,
            Some("0 9 * * *"),
            true,
            Some("2999-01-01T00:00:00Z"),
        ),
    )
    .await;
    // Disabled scheduled routine is filtered out.
    insert(
        &h,
        routine(
            "off",
            RoutineTrigger::Scheduled,
            Some("0 9 * * *"),
            false,
            None,
        ),
    )
    .await;

    h.tick_scheduler().await;

    // The due routine recorded a run and got its next_run computed.
    let runs = h.inner.read().await.routine_runs.clone();
    assert!(runs.iter().any(|r| r.routine_id == "due"));
    assert!(next_run_of(&h, "due").await.is_some());
    // Neither the future-dated nor the disabled routine fired.
    assert!(!runs.iter().any(|r| r.routine_id == "future"));
    assert!(!runs.iter().any(|r| r.routine_id == "off"));
    assert_eq!(
        next_run_of(&h, "future").await.as_deref(),
        Some("2999-01-01T00:00:00Z")
    );
    assert!(next_run_of(&h, "off").await.is_none());
}

#[tokio::test]
async fn spawn_routine_scheduler_does_not_panic() {
    // Exercises the spawn call. The 5s/30s loop body is not driven here.
    let h = WebStateHandle::new_test();
    h.spawn_routine_scheduler();
}
