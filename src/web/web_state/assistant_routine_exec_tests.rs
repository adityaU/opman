//! Tests for routine execution and idle-triggered firing.
use super::*;
use crate::web::web_state::WebStateHandle;

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

fn ensure_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

async fn get_routine(h: &WebStateHandle, id: &str) -> RoutineDefinition {
    h.inner.read().await.routines.get(id).cloned().unwrap()
}

// ── execute_routine ─────────────────────────────────────────────────

#[tokio::test]
async fn execute_routine_not_found() {
    let h = WebStateHandle::new_test();
    let err = h.execute_routine("missing").await.unwrap_err();
    assert_eq!(err, "Routine not found");
}

#[tokio::test]
async fn execute_routine_empty_prompt_fails() {
    let h = WebStateHandle::new_test();
    // SendMessage with no prompt.
    let r = h
        .create_routine(mk_create_routine("np", RoutineTrigger::Manual))
        .await;
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert!(err.contains("no prompt configured"));
}

#[tokio::test]
async fn execute_routine_no_target_session_fails() {
    let h = WebStateHandle::new_test();
    let mut req = mk_create_routine("nt", RoutineTrigger::Manual);
    req.prompt = Some("do stuff".to_string());
    // target_mode None + session_id None → "No target session configured".
    let r = h.create_routine(req).await;
    let err = h.execute_routine(&r.id).await.unwrap_err();
    assert_eq!(err, "No target session configured");
}

#[tokio::test]
async fn execute_routine_existing_session_no_project_fails() {
    let h = WebStateHandle::new_test();
    let mut req = mk_create_routine("ex", RoutineTrigger::Manual);
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
    let mut req = mk_create_routine("ns", RoutineTrigger::Manual);
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
    let mut req = mk_create_routine("ns2", RoutineTrigger::Manual);
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
    let mut req = mk_create_routine("ex2", RoutineTrigger::Manual);
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

// ── try_fire_idle_routines ──────────────────────────────────────────

#[tokio::test]
async fn try_fire_idle_no_matching_routine() {
    let h = WebStateHandle::new_test();
    // A routine bound to a different session should not fire.
    let mut req = mk_create_routine("idle", RoutineTrigger::OnSessionIdle);
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
    let mut req = mk_create_routine("idle", RoutineTrigger::OnSessionIdle);
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
    let mut d = mk_create_routine("disabled", RoutineTrigger::OnSessionIdle);
    d.prompt = Some("x".to_string());
    d.session_id = Some("s".to_string());
    d.enabled = false;
    let _ = h.create_routine(d).await;
    // Wrong trigger.
    let mut w = mk_create_routine("manual", RoutineTrigger::Manual);
    w.prompt = Some("x".to_string());
    w.session_id = Some("s".to_string());
    let _ = h.create_routine(w).await;

    h.try_fire_idle_routines("s").await;
    let (_, runs) = h.list_routines().await;
    assert!(runs.is_empty());
}
