use super::*;
use serde_json::json;

#[test]
fn inbox_item_priority_roundtrip() {
    for (p, s) in [
        (InboxItemPriority::High, "high"),
        (InboxItemPriority::Medium, "medium"),
        (InboxItemPriority::Low, "low"),
    ] {
        assert_eq!(serde_json::to_value(&p).unwrap(), json!(s));
        let back: InboxItemPriority = serde_json::from_value(json!(s)).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json!(s));
        let _ = format!("{p:?}");
        let _ = p.clone();
    }
}

#[test]
fn inbox_item_state_roundtrip() {
    for (st, s) in [
        (InboxItemState::Unresolved, "unresolved"),
        (InboxItemState::Informational, "informational"),
    ] {
        assert_eq!(serde_json::to_value(&st).unwrap(), json!(s));
        let back: InboxItemState = serde_json::from_value(json!(s)).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json!(s));
        let _ = format!("{st:?}");
        let _ = st.clone();
    }
}

#[test]
fn inbox_item_source_roundtrip() {
    for (src, s) in [
        (InboxItemSource::Permission, "permission"),
        (InboxItemSource::Question, "question"),
        (InboxItemSource::Mission, "mission"),
        (InboxItemSource::Watcher, "watcher"),
        (InboxItemSource::Completion, "completion"),
    ] {
        assert_eq!(serde_json::to_value(&src).unwrap(), json!(s));
        let back: InboxItemSource = serde_json::from_value(json!(s)).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json!(s));
        let _ = format!("{src:?}");
        let _ = src.clone();
    }
}

#[test]
fn recommendation_action_serializes_all() {
    for (a, s) in [
        (RecommendationAction::OpenInbox, "open_inbox"),
        (RecommendationAction::OpenMemory, "open_memory"),
        (RecommendationAction::OpenRoutines, "open_routines"),
        (RecommendationAction::OpenDelegation, "open_delegation"),
        (RecommendationAction::OpenWorkspaces, "open_workspaces"),
        (RecommendationAction::OpenAutonomy, "open_autonomy"),
        (
            RecommendationAction::SetupDailySummary,
            "setup_daily_summary",
        ),
        (
            RecommendationAction::UpgradeAutonomyNudge,
            "upgrade_autonomy_nudge",
        ),
        (
            RecommendationAction::SetupDailyCopilot,
            "setup_daily_copilot",
        ),
    ] {
        assert_eq!(serde_json::to_value(&a).unwrap(), json!(s));
        let _ = format!("{a:?}");
        let _ = a.clone();
    }
}

#[test]
fn permission_input_deserializes_with_renames_and_default() {
    let p: PermissionInput = serde_json::from_value(json!({
        "id": "p1",
        "sessionID": "sess",
        "toolName": "Bash",
        "time": 12.5
    }))
    .unwrap();
    assert_eq!(p.id, "p1");
    assert_eq!(p.session_id, "sess");
    assert_eq!(p.tool_name, "Bash");
    assert!(p.description.is_none());
    assert_eq!(p.time, 12.5);

    let p2: PermissionInput = serde_json::from_value(json!({
        "id": "p2",
        "sessionID": "s",
        "toolName": "Edit",
        "description": "edit a file",
        "time": 0.0
    }))
    .unwrap();
    assert_eq!(p2.description, Some("edit a file".into()));
    let _ = format!("{p:?}");
    let _ = p.clone();
}

#[test]
fn question_input_deserializes() {
    let q: QuestionInput = serde_json::from_value(json!({
        "id": "q1",
        "sessionID": "sess",
        "title": "Which option?",
        "time": 3.0
    }))
    .unwrap();
    assert_eq!(q.session_id, "sess");
    assert_eq!(q.title, "Which option?");
    let _ = format!("{q:?}");
    let _ = q.clone();
}

#[test]
fn signal_input_roundtrip_and_default() {
    let s: SignalInput = serde_json::from_value(json!({
        "id": "sig",
        "kind": "info",
        "title": "t",
        "body": "b",
        "created_at": 100.0
    }))
    .unwrap();
    assert!(s.session_id.is_none());
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["kind"], "info");
    assert_eq!(v["created_at"], 100.0);

    let s2: SignalInput = serde_json::from_value(json!({
        "id": "sig",
        "kind": "info",
        "title": "t",
        "body": "b",
        "created_at": 1.0,
        "session_id": "sess"
    }))
    .unwrap();
    assert_eq!(s2.session_id, Some("sess".into()));
    let _ = format!("{s:?}");
    let _ = s.clone();
}

#[test]
fn watcher_status_input_default() {
    let w: WatcherStatusInput = serde_json::from_value(json!({
        "session_id": "s",
        "action": "idle"
    }))
    .unwrap();
    assert_eq!(w.session_id, "s");
    assert_eq!(w.action, "idle");
    assert!(w.idle_since_secs.is_none());
    let w2: WatcherStatusInput = serde_json::from_value(json!({
        "session_id": "s",
        "action": "idle",
        "idle_since_secs": 42
    }))
    .unwrap();
    assert_eq!(w2.idle_since_secs, Some(42));
    let _ = format!("{w:?}");
    let _ = w.clone();
}

#[test]
fn inbox_request_defaults_and_full() {
    let empty: InboxRequest = serde_json::from_value(json!({})).unwrap();
    assert!(empty.permissions.is_empty());
    assert!(empty.questions.is_empty());
    assert!(empty.watcher_status.is_none());
    assert!(empty.signals.is_empty());

    let full: InboxRequest = serde_json::from_value(json!({
        "permissions": [{"id":"p","sessionID":"s","toolName":"T","time":1.0}],
        "questions": [{"id":"q","sessionID":"s","title":"t","time":1.0}],
        "watcher_status": {"session_id":"s","action":"idle"},
        "signals": [{"id":"x","kind":"k","title":"t","body":"b","created_at":1.0}]
    }))
    .unwrap();
    assert_eq!(full.permissions.len(), 1);
    assert_eq!(full.questions.len(), 1);
    assert!(full.watcher_status.is_some());
    assert_eq!(full.signals.len(), 1);
    let _ = format!("{empty:?}");
    let _ = empty.clone();
}

#[test]
fn recommendations_request_defaults() {
    let r: RecommendationsRequest = serde_json::from_value(json!({})).unwrap();
    assert!(r.permissions.is_empty());
    assert!(r.questions.is_empty());
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn session_handoff_request() {
    let r: SessionHandoffRequest = serde_json::from_value(json!({"session_id": "s"})).unwrap();
    assert_eq!(r.session_id, "s");
    assert!(r.permissions.is_empty());
    assert!(r.questions.is_empty());
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn resume_briefing_request_defaults() {
    let r: ResumeBriefingRequest = serde_json::from_value(json!({})).unwrap();
    assert!(r.active_session_id.is_none());
    assert!(r.permissions.is_empty());
    assert!(r.questions.is_empty());
    assert!(r.signals.is_empty());
    let r2: ResumeBriefingRequest =
        serde_json::from_value(json!({"active_session_id": "s"})).unwrap();
    assert_eq!(r2.active_session_id, Some("s".into()));
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn daily_summary_request() {
    let r: DailySummaryRequest = serde_json::from_value(json!({"routine_id": "r1"})).unwrap();
    assert_eq!(r.routine_id, "r1");
    assert!(r.permissions.is_empty());
    assert!(r.questions.is_empty());
    assert!(r.signals.is_empty());
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn add_signal_request() {
    let r: AddSignalRequest = serde_json::from_value(json!({
        "kind": "k",
        "title": "t",
        "body": "b"
    }))
    .unwrap();
    assert!(r.session_id.is_none());
    let r2: AddSignalRequest = serde_json::from_value(json!({
        "kind": "k",
        "title": "t",
        "body": "b",
        "session_id": "s"
    }))
    .unwrap();
    assert_eq!(r2.session_id, Some("s".into()));
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn assistant_center_stats_request_defaults() {
    let r: AssistantCenterStatsRequest = serde_json::from_value(json!({})).unwrap();
    assert!(r.permissions.is_empty());
    assert!(r.questions.is_empty());
    let _ = format!("{r:?}");
    let _ = r.clone();
}

#[test]
fn active_memory_query_defaults() {
    let q: ActiveMemoryQuery = serde_json::from_value(json!({})).unwrap();
    assert!(q.project_index.is_none());
    assert!(q.session_id.is_none());
    let q2: ActiveMemoryQuery =
        serde_json::from_value(json!({"project_index": 4, "session_id": "s"})).unwrap();
    assert_eq!(q2.project_index, Some(4));
    assert_eq!(q2.session_id, Some("s".into()));
    let _ = format!("{q:?}");
    let _ = q.clone();
}
