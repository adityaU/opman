//! Tests for routine CRUD and run records.
use super::*;
use crate::web::web_state::WebStateHandle;

async fn get_routine(h: &WebStateHandle, id: &str) -> RoutineDefinition {
    h.inner.read().await.routines.get(id).cloned().unwrap()
}

// ── Routines ────────────────────────────────────────────────────────

fn mk_create_routine(name: &str, trigger: RoutineTrigger) -> CreateRoutineRequest {
    CreateRoutineRequest {
        name: name.to_string(),
        trigger,
        enabled: true,
        cron_expr: None,
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
    }
}

#[tokio::test]
async fn routines_list_empty() {
    let h = WebStateHandle::new_test();
    let (routines, runs) = h.list_routines().await;
    assert!(routines.is_empty());
    assert!(runs.is_empty());
}

#[tokio::test]
async fn routine_create_update_delete() {
    let h = WebStateHandle::new_test();
    let r = h
        .create_routine(mk_create_routine("daily", RoutineTrigger::Manual))
        .await;
    assert!(r.id.starts_with("routine-"));
    assert_eq!(r.name, "daily");
    assert!(r.enabled);

    let (routines, _) = h.list_routines().await;
    assert_eq!(routines.len(), 1);

    // Update every field group.
    let upd = UpdateRoutineRequest {
        name: Some("renamed".to_string()),
        trigger: Some(RoutineTrigger::OnSessionIdle),
        enabled: Some(false),
        cron_expr: Some(Some("* * * * *".to_string())),
        timezone: Some(Some("UTC".to_string())),
        target_mode: Some(Some(RoutineTargetMode::NewSession)),
        session_id: Some(Some("sess".to_string())),
        project_index: Some(Some(1)),
        prompt: Some(Some("hi".to_string())),
        provider_id: Some(Some("anthropic".to_string())),
        model_id: Some(Some("claude".to_string())),
    };
    let updated = h.update_routine(&r.id, upd).await.unwrap();
    assert_eq!(updated.name, "renamed");
    assert_eq!(updated.trigger, RoutineTrigger::OnSessionIdle);
    assert!(!updated.enabled);
    assert_eq!(updated.cron_expr.as_deref(), Some("* * * * *"));
    assert_eq!(updated.timezone.as_deref(), Some("UTC"));
    assert_eq!(updated.target_mode, Some(RoutineTargetMode::NewSession));
    assert_eq!(updated.session_id.as_deref(), Some("sess"));
    assert_eq!(updated.project_index, Some(1));
    assert_eq!(updated.prompt.as_deref(), Some("hi"));
    assert_eq!(updated.provider_id.as_deref(), Some("anthropic"));
    assert_eq!(updated.model_id.as_deref(), Some("claude"));

    assert!(h.delete_routine(&r.id).await);
    assert!(!h.delete_routine(&r.id).await);
}

#[tokio::test]
async fn routine_update_not_found() {
    let h = WebStateHandle::new_test();
    let upd = UpdateRoutineRequest {
        name: None,
        trigger: None,
        enabled: None,
        cron_expr: None,
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
    };
    assert!(h.update_routine("missing", upd).await.is_none());
}

#[tokio::test]
async fn routine_update_scheduled_recomputes_next_run() {
    let h = WebStateHandle::new_test();
    let r = h
        .create_routine(mk_create_routine("sched", RoutineTrigger::Scheduled))
        .await;
    // enabled + Scheduled + cron_expr set → recompute_next_run_if_scheduled fires update.
    let upd = UpdateRoutineRequest {
        name: None,
        trigger: None,
        enabled: Some(true),
        cron_expr: Some(Some("0 0 * * *".to_string())),
        timezone: Some(Some("UTC".to_string())),
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
    };
    h.update_routine(&r.id, upd).await.unwrap();
    // update_routine returns a snapshot taken before recompute_next_run_if_scheduled
    // runs, so read the stored routine to observe the recomputed next_run_at.
    let next = h
        .inner
        .read()
        .await
        .routines
        .get(&r.id)
        .unwrap()
        .next_run_at
        .clone();
    assert!(next.is_some());
}

#[tokio::test]
async fn routine_list_sorted_by_updated_desc() {
    let h = WebStateHandle::new_test();
    let a = h
        .create_routine(mk_create_routine("a", RoutineTrigger::Manual))
        .await;
    let _b = h
        .create_routine(mk_create_routine("b", RoutineTrigger::Manual))
        .await;
    {
        let mut s = h.inner.write().await;
        s.routines.get_mut(&a.id).unwrap().updated_at = "2999-01-01T00:00:00Z".to_string();
    }
    let (routines, _) = h.list_routines().await;
    assert_eq!(routines[0].id, a.id);
}

// ── record_routine_run ──────────────────────────────────────────────

#[tokio::test]
async fn record_run_completed_and_failed_updates_routine() {
    let h = WebStateHandle::new_test();
    let r = h
        .create_routine(mk_create_routine("r", RoutineTrigger::Manual))
        .await;

    let run = h
        .record_routine_run(
            &r.id,
            "ok".to_string(),
            Some("s".to_string()),
            Some(12),
            "completed",
        )
        .await;
    assert_eq!(run.status, "completed");
    let after = get_routine(&h, &r.id).await;
    assert!(after.last_run_at.is_some());
    assert!(after.last_error.is_none());

    let _ = h
        .record_routine_run(&r.id, "boom".to_string(), None, None, "failed")
        .await;
    let after2 = get_routine(&h, &r.id).await;
    assert_eq!(after2.last_error.as_deref(), Some("boom"));
}

#[tokio::test]
async fn record_run_unknown_routine_still_recorded() {
    let h = WebStateHandle::new_test();
    let run = h
        .record_routine_run("ghost", "s".to_string(), None, None, "completed")
        .await;
    assert_eq!(run.routine_id, "ghost");
    let (_, runs) = h.list_routines().await;
    assert_eq!(runs.len(), 1);
}

#[tokio::test]
async fn record_run_truncates_to_100() {
    let h = WebStateHandle::new_test();
    // Pre-fill 100 dummy runs.
    {
        let mut s = h.inner.write().await;
        for i in 0..100 {
            s.routine_runs.push(RoutineRunRecord {
                id: format!("run-{i}"),
                routine_id: "x".to_string(),
                status: "completed".to_string(),
                summary: "s".to_string(),
                target_session_id: None,
                duration_ms: None,
                created_at: Utc::now().to_rfc3339(),
            });
        }
    }
    let _ = h
        .record_routine_run("x", "newest".to_string(), None, None, "completed")
        .await;
    let (_, runs) = h.list_routines().await;
    assert_eq!(runs.len(), 100);
    assert_eq!(runs[0].summary, "newest"); // inserted at front
}
