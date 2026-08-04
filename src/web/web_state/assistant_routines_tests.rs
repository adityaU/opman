//! Generated tests for assistant.rs — part 2: mission-loop orchestration,
//! personal memory, autonomy settings, and routines.
use super::*;
use crate::web::web_state::WebStateHandle;

fn ensure_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

fn mk_mission(id: &str, session: &str, state: MissionState, iteration: u32, max: u32) -> Mission {
    let now = Utc::now().to_rfc3339();
    Mission {
        id: id.to_string(),
        goal: "reach the goal".to_string(),
        session_id: session.to_string(),
        project_index: 0,
        state,
        iteration,
        max_iterations: max,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

async fn insert_mission(h: &WebStateHandle, m: Mission) {
    let mut s = h.inner.write().await;
    s.missions.insert(m.id.clone(), m);
}

async fn get_state(h: &WebStateHandle, id: &str) -> MissionState {
    h.get_mission(id).await.unwrap().state
}

async fn get_routine(h: &WebStateHandle, id: &str) -> RoutineDefinition {
    h.inner.read().await.routines.get(id).cloned().unwrap()
}

// ── on_mission_session_idle ─────────────────────────────────────────

#[tokio::test]
async fn on_idle_no_matching_mission_returns() {
    let h = WebStateHandle::new_test();
    // No missions at all → early return, no panic.
    h.on_mission_session_idle("sess").await;
}

#[tokio::test]
async fn on_idle_executing_transitions_to_evaluating() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Executing, 1, 10)).await;
    h.on_mission_session_idle("sess").await;
    assert_eq!(get_state(&h, "m1").await, MissionState::Evaluating);
}

#[tokio::test]
async fn on_idle_executing_unlimited_iterations() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    // max_iterations 0 exercises the "∞" branch of send_evaluator_prompt.
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Executing, 3, 0)).await;
    h.on_mission_session_idle("sess").await;
    assert_eq!(get_state(&h, "m1").await, MissionState::Evaluating);
}

#[tokio::test]
async fn on_idle_ignores_non_executing() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Paused, 1, 10)).await;
    h.on_mission_session_idle("sess").await;
    assert_eq!(get_state(&h, "m1").await, MissionState::Paused); // unchanged
}

// ── on_mission_evaluation_complete ──────────────────────────────────

#[tokio::test]
async fn on_eval_complete_no_match() {
    let h = WebStateHandle::new_test();
    h.on_mission_evaluation_complete("sess").await; // no mission → return
}

#[tokio::test]
async fn on_eval_complete_continue_under_max() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    // No project → parse_latest_eval_response yields Continue (default).
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 1, 5)).await;
    h.on_mission_evaluation_complete("sess").await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Executing);
    assert_eq!(m.iteration, 2);
    assert_eq!(m.eval_history.len(), 1);
    assert!(m.last_verdict.is_some());
    assert!(m.last_eval_summary.is_some());
}

#[tokio::test]
async fn on_eval_complete_continue_over_max_fails() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    // iteration == max → next_iter (max+1) > max → Failed.
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 5, 5)).await;
    h.on_mission_evaluation_complete("sess").await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Failed);
    assert_eq!(m.iteration, 5);
}

#[tokio::test]
async fn on_eval_complete_continue_unlimited() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    // max_iterations 0 → never auto-fail → Executing.
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 9, 0)).await;
    h.on_mission_evaluation_complete("sess").await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Executing);
    assert_eq!(m.iteration, 10);
}

// ── try_advance_mission ─────────────────────────────────────────────

#[tokio::test]
async fn try_advance_executing_routes_to_idle() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Executing, 1, 10)).await;
    h.try_advance_mission("sess").await;
    assert_eq!(get_state(&h, "m1").await, MissionState::Evaluating);
}

#[tokio::test]
async fn try_advance_evaluating_routes_to_eval_complete() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    insert_mission(
        &h,
        mk_mission("m1", "sess", MissionState::Evaluating, 1, 10),
    )
    .await;
    h.try_advance_mission("sess").await;
    // Continue verdict → Executing, iteration bumped.
    assert_eq!(get_state(&h, "m1").await, MissionState::Executing);
}

#[tokio::test]
async fn try_advance_no_active_mission() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Pending, 0, 10)).await;
    h.try_advance_mission("sess").await; // Pending is not active → no-op
    assert_eq!(get_state(&h, "m1").await, MissionState::Pending);
}

// ── Personal Memory ─────────────────────────────────────────────────

fn mk_create_memory(label: &str) -> CreatePersonalMemoryRequest {
    CreatePersonalMemoryRequest {
        label: label.to_string(),
        content: "content".to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    }
}

#[tokio::test]
async fn personal_memory_crud_lifecycle() {
    let h = WebStateHandle::new_test();
    assert!(h.list_personal_memory().await.is_empty());

    let item = h.create_personal_memory(mk_create_memory("first")).await;
    assert!(item.id.starts_with("memory-"));
    assert_eq!(item.label, "first");
    assert_eq!(h.list_personal_memory().await.len(), 1);

    // Update all fields.
    let upd = UpdatePersonalMemoryRequest {
        label: Some("renamed".to_string()),
        content: Some("new content".to_string()),
        scope: Some(MemoryScope::Project),
        project_index: Some(Some(2)),
        session_id: Some(Some("s".to_string())),
    };
    let updated = h.update_personal_memory(&item.id, upd).await.unwrap();
    assert_eq!(updated.label, "renamed");
    assert_eq!(updated.content, "new content");
    assert!(matches!(updated.scope, MemoryScope::Project));
    assert_eq!(updated.project_index, Some(2));
    assert_eq!(updated.session_id.as_deref(), Some("s"));

    assert!(h.delete_personal_memory(&item.id).await);
    assert!(!h.delete_personal_memory(&item.id).await);
}

#[tokio::test]
async fn personal_memory_update_not_found_and_no_fields() {
    let h = WebStateHandle::new_test();
    let none_req = UpdatePersonalMemoryRequest {
        label: None,
        content: None,
        scope: None,
        project_index: None,
        session_id: None,
    };
    assert!(h
        .update_personal_memory("missing", none_req)
        .await
        .is_none());

    let item = h.create_personal_memory(mk_create_memory("x")).await;
    let none_req2 = UpdatePersonalMemoryRequest {
        label: None,
        content: None,
        scope: None,
        project_index: None,
        session_id: None,
    };
    let unchanged = h.update_personal_memory(&item.id, none_req2).await.unwrap();
    assert_eq!(unchanged.label, "x");
}

#[tokio::test]
async fn personal_memory_list_sorted_by_updated_desc() {
    let h = WebStateHandle::new_test();
    let a = h.create_personal_memory(mk_create_memory("a")).await;
    let _b = h.create_personal_memory(mk_create_memory("b")).await;
    // Force a's updated_at to be newer.
    {
        let mut s = h.inner.write().await;
        s.personal_memory.get_mut(&a.id).unwrap().updated_at = "2999-01-01T00:00:00Z".to_string();
    }
    let list = h.list_personal_memory().await;
    assert_eq!(list[0].id, a.id);
}

// ── Autonomy ────────────────────────────────────────────────────────

#[tokio::test]
async fn autonomy_get_default_and_update() {
    let h = WebStateHandle::new_test();
    // Default is retrievable.
    let _ = h.get_autonomy_settings().await;
    let updated = h.update_autonomy_settings(AutonomyMode::Autonomous).await;
    assert!(matches!(updated.mode, AutonomyMode::Autonomous));
    let got = h.get_autonomy_settings().await;
    assert!(matches!(got.mode, AutonomyMode::Autonomous));

    // Cover the other modes too.
    for mode in [
        AutonomyMode::Observe,
        AutonomyMode::Nudge,
        AutonomyMode::Continue,
    ] {
        let out = h.update_autonomy_settings(mode).await;
        assert!(!out.updated_at.is_empty());
    }
}

// ── Routines ────────────────────────────────────────────────────────

fn mk_create_routine(
    name: &str,
    trigger: RoutineTrigger,
    action: RoutineAction,
) -> CreateRoutineRequest {
    CreateRoutineRequest {
        name: name.to_string(),
        trigger,
        action,
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
        .create_routine(mk_create_routine(
            "daily",
            RoutineTrigger::Manual,
            RoutineAction::SendMessage,
        ))
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
        action: Some(RoutineAction::OpenInbox),
        enabled: Some(false),
        cron_expr: Some(Some("* * * * *".to_string())),
        timezone: Some(Some("UTC".to_string())),
        target_mode: Some(Some(RoutineTargetMode::NewSession)),
        session_id: Some(Some("sess".to_string())),
        project_index: Some(Some(1)),
        prompt: Some(Some("hi".to_string())),
        provider_id: Some(Some("anthropic".to_string())),
        model_id: Some(Some("claude".to_string())),
        mission_id: Some(Some("m1".to_string())),
    };
    let updated = h.update_routine(&r.id, upd).await.unwrap();
    assert_eq!(updated.name, "renamed");
    assert_eq!(updated.trigger, RoutineTrigger::OnSessionIdle);
    assert_eq!(updated.action, RoutineAction::OpenInbox);
    assert!(!updated.enabled);
    assert_eq!(updated.cron_expr.as_deref(), Some("* * * * *"));
    assert_eq!(updated.prompt.as_deref(), Some("hi"));

    assert!(h.delete_routine(&r.id).await);
    assert!(!h.delete_routine(&r.id).await);
}

#[tokio::test]
async fn routine_update_not_found() {
    let h = WebStateHandle::new_test();
    let upd = UpdateRoutineRequest {
        name: None,
        trigger: None,
        action: None,
        enabled: None,
        cron_expr: None,
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
        mission_id: None,
    };
    assert!(h.update_routine("missing", upd).await.is_none());
}

#[tokio::test]
async fn routine_update_scheduled_recomputes_next_run() {
    let h = WebStateHandle::new_test();
    let r = h
        .create_routine(mk_create_routine(
            "sched",
            RoutineTrigger::Scheduled,
            RoutineAction::SendMessage,
        ))
        .await;
    // enabled + Scheduled + cron_expr set → recompute_next_run_if_scheduled fires update.
    let upd = UpdateRoutineRequest {
        name: None,
        trigger: None,
        action: None,
        enabled: Some(true),
        cron_expr: Some(Some("0 0 * * *".to_string())),
        timezone: Some(Some("UTC".to_string())),
        target_mode: None,
        session_id: None,
        project_index: None,
        prompt: None,
        provider_id: None,
        model_id: None,
        mission_id: None,
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
        .create_routine(mk_create_routine(
            "a",
            RoutineTrigger::Manual,
            RoutineAction::SendMessage,
        ))
        .await;
    let _b = h
        .create_routine(mk_create_routine(
            "b",
            RoutineTrigger::Manual,
            RoutineAction::SendMessage,
        ))
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
        .create_routine(mk_create_routine(
            "r",
            RoutineTrigger::Manual,
            RoutineAction::SendMessage,
        ))
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

// ── execute_routine ─────────────────────────────────────────────────

#[tokio::test]
async fn execute_routine_not_found() {
    let h = WebStateHandle::new_test();
    let err = h.execute_routine("missing").await.unwrap_err();
    assert_eq!(err, "Routine not found");
}

#[tokio::test]
async fn execute_routine_legacy_action_records_completed() {
    let h = WebStateHandle::new_test();
    let r = h
        .create_routine(mk_create_routine(
            "legacy",
            RoutineTrigger::Manual,
            RoutineAction::ReviewMission,
        ))
        .await;
    let run = h.execute_routine(&r.id).await.unwrap();
    assert_eq!(run.status, "completed");
    assert!(run.summary.contains("legacy action"));
}

#[tokio::test]
async fn execute_routine_empty_prompt_fails() {
    let h = WebStateHandle::new_test();
    // SendMessage with no prompt.
    let r = h
        .create_routine(mk_create_routine(
            "np",
            RoutineTrigger::Manual,
            RoutineAction::SendMessage,
        ))
        .await;
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert!(err.contains("no prompt configured"));
}

#[tokio::test]
async fn execute_routine_no_target_session_fails() {
    let h = WebStateHandle::new_test();
    let mut req = mk_create_routine("nt", RoutineTrigger::Manual, RoutineAction::SendMessage);
    req.prompt = Some("do stuff".to_string());
    // target_mode None + session_id None → "No target session configured".
    let r = h.create_routine(req).await;
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert_eq!(err, "No target session configured");
}

#[tokio::test]
async fn execute_routine_existing_session_no_project_fails() {
    let h = WebStateHandle::new_test();
    let mut req = mk_create_routine("ex", RoutineTrigger::Manual, RoutineAction::SendMessage);
    req.prompt = Some("do stuff".to_string());
    req.target_mode = Some(RoutineTargetMode::ExistingSession);
    req.session_id = Some("sess-existing".to_string());
    let r = h.create_routine(req).await;
    // No project → send_to_session returns Err on empty dir.
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert_eq!(err, "No project directory found");
}

#[tokio::test]
async fn execute_routine_new_session_no_project_fails() {
    let h = WebStateHandle::new_test();
    let mut req = mk_create_routine("ns", RoutineTrigger::Manual, RoutineAction::SendMessage);
    req.prompt = Some("do stuff".to_string());
    req.target_mode = Some(RoutineTargetMode::NewSession);
    req.project_index = Some(0);
    let r = h.create_routine(req).await;
    // No project → create_session_for_routine returns Err on empty dir.
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert_eq!(err, "No project directory found");
}

#[tokio::test]
async fn execute_routine_new_session_with_project_connection_refused() {
    ensure_base_url();
    let dir = std::env::temp_dir();
    let h = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    let mut req = mk_create_routine("ns2", RoutineTrigger::Manual, RoutineAction::SendMessage);
    req.prompt = Some("do stuff".to_string());
    req.target_mode = Some(RoutineTargetMode::NewSession);
    req.project_index = Some(0);
    let r = h.create_routine(req).await;
    // create_session_for_routine reaches reqwest → connection refused.
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert!(err.starts_with("Failed to create session"));
}

#[tokio::test]
async fn execute_routine_existing_session_with_project_and_model_refused() {
    ensure_base_url();
    let dir = std::env::temp_dir();
    let h = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    let mut req = mk_create_routine("ex2", RoutineTrigger::Manual, RoutineAction::SendMessage);
    req.prompt = Some("do stuff".to_string());
    req.target_mode = Some(RoutineTargetMode::ExistingSession);
    req.session_id = Some("sess-existing-long-id".to_string());
    req.project_index = Some(0);
    req.provider_id = Some("anthropic".to_string());
    req.model_id = Some("claude".to_string());
    let r = h.create_routine(req).await;
    // send_to_session reaches reqwest (with model override) → connection refused.
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert!(err.starts_with("Failed to send message"));
    // A failed run should have been recorded.
    let (_, runs) = h.list_routines().await;
    assert!(runs.iter().any(|r| r.status == "failed"));
}

// ── create_session_for_routine (direct) ─────────────────────────────

#[tokio::test]
async fn create_session_for_routine_empty_dir() {
    let h = WebStateHandle::new_test();
    let err = h.create_session_for_routine(0).await.unwrap_err();
    assert_eq!(err, "No project directory found");
}

#[tokio::test]
async fn create_session_for_routine_connection_refused() {
    ensure_base_url();
    let dir = std::env::temp_dir();
    let h = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    let err = h.create_session_for_routine(0).await.unwrap_err();
    assert!(err.starts_with("Failed to create session"));
}

// ── try_fire_idle_routines ──────────────────────────────────────────

#[tokio::test]
async fn try_fire_idle_no_matching_routine() {
    let h = WebStateHandle::new_test();
    // A routine bound to a different session should not fire.
    let mut req = mk_create_routine(
        "idle",
        RoutineTrigger::OnSessionIdle,
        RoutineAction::SendMessage,
    );
    req.prompt = Some("ping".to_string());
    req.session_id = Some("other-sess".to_string());
    let _ = h.create_routine(req).await;
    h.try_fire_idle_routines("target-sess").await;
    let (_, runs) = h.list_routines().await;
    assert!(runs.is_empty()); // nothing fired
}

#[tokio::test]
async fn try_fire_idle_fires_and_respects_cooldown() {
    let h = WebStateHandle::new_test();
    let mut req = mk_create_routine(
        "idle",
        RoutineTrigger::OnSessionIdle,
        RoutineAction::SendMessage,
    );
    req.prompt = Some("ping".to_string());
    req.session_id = Some("sess-idle".to_string());
    // No project → execute fails, but the run is still recorded and cooldown set.
    let _ = h.create_routine(req).await;

    h.try_fire_idle_routines("sess-idle").await;
    let (_, runs1) = h.list_routines().await;
    assert_eq!(runs1.len(), 1);

    // Second fire within cooldown window is suppressed → no new run.
    h.try_fire_idle_routines("sess-idle").await;
    let (_, runs2) = h.list_routines().await;
    assert_eq!(runs2.len(), 1);
}

#[tokio::test]
async fn try_fire_idle_skips_disabled_and_wrong_trigger() {
    let h = WebStateHandle::new_test();
    // Disabled routine.
    let mut d = mk_create_routine(
        "disabled",
        RoutineTrigger::OnSessionIdle,
        RoutineAction::SendMessage,
    );
    d.prompt = Some("x".to_string());
    d.session_id = Some("s".to_string());
    d.enabled = false;
    let _ = h.create_routine(d).await;
    // Wrong trigger.
    let mut w = mk_create_routine("manual", RoutineTrigger::Manual, RoutineAction::SendMessage);
    w.prompt = Some("x".to_string());
    w.session_id = Some("s".to_string());
    let _ = h.create_routine(w).await;

    h.try_fire_idle_routines("s").await;
    let (_, runs) = h.list_routines().await;
    assert!(runs.is_empty());
}
