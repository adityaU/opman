use super::*;
use serde_json::json;

#[test]
fn autonomy_mode_roundtrip_all_variants() {
    for (m, s) in [
        (AutonomyMode::Observe, "observe"),
        (AutonomyMode::Nudge, "nudge"),
        (AutonomyMode::Continue, "continue"),
        (AutonomyMode::Autonomous, "autonomous"),
    ] {
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v, json!(s));
        let back: AutonomyMode = serde_json::from_value(json!(s)).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json!(s));
        let _ = format!("{m:?}");
        let _ = m.clone();
    }
}

#[test]
fn autonomy_settings_roundtrip() {
    let s = AutonomySettings {
        mode: AutonomyMode::Nudge,
        updated_at: "2026-01-01".into(),
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["mode"], "nudge");
    assert_eq!(v["updated_at"], "2026-01-01");
    let back: AutonomySettings = serde_json::from_value(v).unwrap();
    assert_eq!(back.updated_at, "2026-01-01");
    let _ = format!("{s:?}");
    let _ = s.clone();
}

#[test]
fn update_autonomy_settings_request_deserializes() {
    let r: UpdateAutonomySettingsRequest =
        serde_json::from_value(json!({"mode": "autonomous"})).unwrap();
    assert!(matches!(r.mode, AutonomyMode::Autonomous));
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn routine_trigger_variants_roundtrip() {
    for (t, s) in [
        (RoutineTrigger::Manual, "manual"),
        (RoutineTrigger::Scheduled, "scheduled"),
        (RoutineTrigger::OnSessionIdle, "on_session_idle"),
        (RoutineTrigger::DailySummary, "daily_summary"),
    ] {
        assert_eq!(serde_json::to_value(&t).unwrap(), json!(s));
        let back: RoutineTrigger = serde_json::from_value(json!(s)).unwrap();
        assert_eq!(back, t);
        let _ = format!("{t:?}");
        let _ = t.clone();
    }
}

#[test]
fn routine_target_mode_roundtrip() {
    for (tm, s) in [
        (RoutineTargetMode::ExistingSession, "existing_session"),
        (RoutineTargetMode::NewSession, "new_session"),
    ] {
        assert_eq!(serde_json::to_value(&tm).unwrap(), json!(s));
        let back: RoutineTargetMode = serde_json::from_value(json!(s)).unwrap();
        assert_eq!(back, tm);
    }
}

#[test]
fn routine_definition_defaults_apply() {
    // Only required fields provided; defaults fill the rest.
    let def: RoutineDefinition = serde_json::from_value(json!({
        "id": "r1",
        "name": "My Routine",
        "trigger": "manual",
        "created_at": "c",
        "updated_at": "u"
    }))
    .unwrap();
    assert!(def.enabled, "enabled defaults to true");
    assert!(def.cron_expr.is_none());
    assert!(def.timezone.is_none());
    assert!(def.target_mode.is_none());
    assert!(def.session_id.is_none());
    assert!(def.project_index.is_none());
    assert!(def.prompt.is_none());
    assert!(def.provider_id.is_none());
    assert!(def.model_id.is_none());
    assert!(def.last_run_at.is_none());
    assert!(def.next_run_at.is_none());
    assert!(def.last_error.is_none());
    let _ = format!("{def:?}");
    let _ = def.clone();
}

#[test]
fn routine_definition_full_roundtrip() {
    let def = RoutineDefinition {
        id: "r1".into(),
        name: "n".into(),
        trigger: RoutineTrigger::Scheduled,
        enabled: false,
        cron_expr: Some("* * * * *".into()),
        timezone: Some("UTC".into()),
        target_mode: Some(RoutineTargetMode::NewSession),
        session_id: Some("s".into()),
        project_index: Some(3),
        prompt: Some("hi".into()),
        provider_id: Some("anthropic".into()),
        model_id: Some("claude".into()),
        last_run_at: Some("l".into()),
        next_run_at: Some("nx".into()),
        last_error: Some("err".into()),
        created_at: "c".into(),
        updated_at: "u".into(),
    };
    let v = serde_json::to_value(&def).unwrap();
    assert_eq!(v["enabled"], false);
    assert_eq!(v["cron_expr"], "* * * * *");
    assert_eq!(v["project_index"], 3);
    let back: RoutineDefinition = serde_json::from_value(v).unwrap();
    assert_eq!(back.trigger, RoutineTrigger::Scheduled);
    assert_eq!(back.project_index, Some(3));
}

#[test]
fn routine_run_record_defaults_and_roundtrip() {
    let rec: RoutineRunRecord = serde_json::from_value(json!({
        "id": "run1",
        "routine_id": "r1",
        "status": "completed",
        "summary": "done",
        "created_at": "c"
    }))
    .unwrap();
    assert!(rec.target_session_id.is_none());
    assert!(rec.duration_ms.is_none());
    let full = RoutineRunRecord {
        id: "x".into(),
        routine_id: "r".into(),
        status: "failed".into(),
        summary: "s".into(),
        target_session_id: Some("t".into()),
        duration_ms: Some(1234),
        created_at: "c".into(),
    };
    let v = serde_json::to_value(&full).unwrap();
    assert_eq!(v["duration_ms"], 1234);
    let _ = format!("{rec:?}{full:?}");
    let _ = rec.clone();
    let _ = full.clone();
}

#[test]
fn create_routine_request_defaults() {
    let req: CreateRoutineRequest = serde_json::from_value(json!({
        "name": "n",
        "trigger": "manual",
    }))
    .unwrap();
    assert!(req.enabled);
    assert!(req.cron_expr.is_none());
    assert!(req.project_index.is_none());
    let _ = format!("{req:?}");
    let _ = req.clone();
}

#[test]
fn create_routine_request_full() {
    let req: CreateRoutineRequest = serde_json::from_value(json!({
        "name": "n",
        "trigger": "scheduled",
        "enabled": false,
        "cron_expr": "* * * * *",
        "timezone": "UTC",
        "target_mode": "existing_session",
        "session_id": "s",
        "project_index": 2,
        "prompt": "p",
        "provider_id": "prov",
        "model_id": "mod",
    }))
    .unwrap();
    assert!(!req.enabled);
    assert_eq!(req.project_index, Some(2));
    assert_eq!(req.target_mode, Some(RoutineTargetMode::ExistingSession));
}

#[test]
fn update_routine_request_double_option_semantics() {
    // These are plain `Option<Option<T>>` with `#[serde(default)]` (no
    // double_option deserializer), so serde maps a present `null` to the OUTER
    // None — indistinguishable from omitted. Present value → Some(Some).
    let omitted: UpdateRoutineRequest = serde_json::from_value(json!({})).unwrap();
    assert!(omitted.name.is_none());
    assert!(omitted.cron_expr.is_none());

    let explicit_null: UpdateRoutineRequest =
        serde_json::from_value(json!({"cron_expr": null, "prompt": null})).unwrap();
    assert_eq!(explicit_null.cron_expr, None);
    assert_eq!(explicit_null.prompt, None);

    let set: UpdateRoutineRequest = serde_json::from_value(json!({
        "name": "new",
        "trigger": "manual",
        "enabled": true,
        "cron_expr": "1 * * * *",
        "timezone": "UTC",
        "target_mode": "new_session",
        "session_id": "s",
        "project_index": 1,
        "prompt": "p",
        "provider_id": "pr",
        "model_id": "m",
    }))
    .unwrap();
    assert_eq!(set.name, Some("new".into()));
    assert_eq!(set.cron_expr, Some(Some("1 * * * *".into())));
    assert_eq!(set.project_index, Some(Some(1)));
    assert_eq!(set.enabled, Some(true));
    let _ = format!("{set:?}");
    let _ = set.clone();
}

#[test]
fn run_routine_request_default_and_value() {
    let empty: RunRoutineRequest = serde_json::from_value(json!({})).unwrap();
    assert!(empty.summary.is_none());
    let some: RunRoutineRequest = serde_json::from_value(json!({"summary": "hi"})).unwrap();
    assert_eq!(some.summary, Some("hi".into()));
    let _ = format!("{empty:?}");
    let _ = empty.clone();
}

#[test]
fn routines_list_response_serializes() {
    let resp = RoutinesListResponse {
        routines: vec![],
        runs: vec![],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["routines"], json!([]));
    assert_eq!(v["runs"], json!([]));
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}

#[test]
fn default_true_helper() {
    assert!(default_true());
}
