use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

fn activity(sid: &str, summary: &str, ts: &str) -> ActivityEventPayload {
    ActivityEventPayload {
        session_id: sid.to_string(),
        kind: "status".to_string(),
        summary: summary.to_string(),
        detail: None,
        timestamp: ts.to_string(),
    }
}

fn perm(id: &str, sid: &str, tool: &str) -> PermissionInput {
    PermissionInput {
        id: id.to_string(),
        session_id: sid.to_string(),
        tool_name: tool.to_string(),
        description: None,
        time: 1.0,
    }
}

fn question(id: &str, sid: &str, title: &str) -> QuestionInput {
    QuestionInput {
        id: id.to_string(),
        session_id: sid.to_string(),
        title: title.to_string(),
        time: 1.0,
    }
}

fn signal(id: &str, title: &str) -> SignalInput {
    SignalInput {
        id: id.to_string(),
        kind: "k".to_string(),
        title: title.to_string(),
        body: "b".to_string(),
        created_at: 1.0,
        session_id: None,
    }
}

/// Insert a mission directly into inner state with an explicit lifecycle state.
async fn seed_mission(h: &WebStateHandle, id: &str, session_id: &str, state: MissionState) {
    let m = Mission {
        id: id.to_string(),
        goal: "reach the goal".to_string(),
        session_id: session_id.to_string(),
        project_index: 0,
        state,
        iteration: 2,
        max_iterations: 5,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    h.inner.write().await.missions.insert(id.to_string(), m);
}

// ── build_session_handoff ────────────────────────────────────────────

#[tokio::test]
async fn session_handoff_empty_id_returns_none() {
    let h = WebStateHandle::new_test();
    let out = h
        .build_session_handoff(SessionHandoffRequest {
            session_id: String::new(),
            permissions: vec![],
            questions: vec![],
        })
        .await;
    assert!(out.is_none());
}

#[tokio::test]
async fn session_handoff_no_activity_no_blockers() {
    let h = WebStateHandle::new_test();
    let out = h
        .build_session_handoff(SessionHandoffRequest {
            session_id: "abcdefghxyz".to_string(),
            permissions: vec![],
            questions: vec![],
        })
        .await
        .unwrap();
    assert_eq!(out.title, "Session abcdefgh");
    assert_eq!(out.summary, "Session abcdefgh");
    assert!(out.blockers.is_empty());
    // recent empty → recent_changes falls back to [summary].
    assert_eq!(out.recent_changes, vec!["Session abcdefgh".to_string()]);
    assert_eq!(out.next_action, "Continue session");
    assert!(out.links.is_empty());
}

#[tokio::test]
async fn session_handoff_with_activity_and_blockers() {
    let h = WebStateHandle::new_test();
    let sid = "sess1234";
    h.push_activity_event(activity(sid, "old event", "2024-01-01T00:00:01Z"))
        .await;
    h.push_activity_event(activity(sid, "newest event", "2024-01-01T00:00:02Z"))
        .await;

    let out = h
        .build_session_handoff(SessionHandoffRequest {
            session_id: sid.to_string(),
            permissions: vec![
                perm("p1", sid, "bash"),
                perm("p2", "other-session", "edit"), // filtered out (different session)
            ],
            questions: vec![question("q1", sid, "choose")],
        })
        .await
        .unwrap();
    // Summary comes from the most recent event (activity reversed → newest first).
    assert_eq!(out.summary, "newest event");
    assert!(!out.recent_changes.is_empty());
    // Blockers: one permission (matching session) + one question.
    assert_eq!(out.blockers.len(), 2);
    assert_eq!(out.blockers[0], "Permission needed: bash");
    assert_eq!(out.blockers[1], "Question pending: choose");
    // next_action = first blocker.
    assert_eq!(out.next_action, "Permission needed: bash");
    // links: one for perm, one for question.
    assert_eq!(out.links.len(), 2);
    assert_eq!(out.links[0].kind, "permission");
    assert_eq!(out.links[0].source_id.as_deref(), Some("p1"));
    assert_eq!(out.links[1].kind, "question");
    assert_eq!(out.links[1].source_id.as_deref(), Some("q1"));
}

// ── build_resume_briefing ────────────────────────────────────────────

#[tokio::test]
async fn resume_briefing_none_when_nothing() {
    let h = WebStateHandle::new_test();
    // active_session_id None → empty → session_brief None; no missions; no signals.
    let out = h
        .build_resume_briefing(ResumeBriefingRequest {
            active_session_id: None,
            permissions: vec![],
            questions: vec![],
            signals: vec![],
        })
        .await;
    assert!(out.is_none());
}

#[tokio::test]
async fn resume_briefing_with_session_mission_and_signals() {
    let h = WebStateHandle::new_test();
    let sid = "session-abc";
    h.push_activity_event(activity(sid, "did work", "2024-01-01T00:00:01Z"))
        .await;
    seed_mission(&h, "m1", sid, MissionState::Executing).await;

    let out = h
        .build_resume_briefing(ResumeBriefingRequest {
            active_session_id: Some(sid.to_string()),
            permissions: vec![],
            questions: vec![],
            signals: vec![
                signal("s1", "sig one"),
                signal("s2", "sig two"),
                signal("s3", "ignored"),
            ],
        })
        .await
        .unwrap();
    assert_eq!(out.title, "Session session-");
    // Summary includes mission context prefix and the (max 2) signal titles.
    assert!(out.summary.contains("executing, iteration 2/5"));
    assert!(out.summary.contains("did work"));
    assert!(out.summary.contains("sig one"));
    assert!(out.summary.contains("sig two"));
    assert!(!out.summary.contains("ignored"));
}

#[tokio::test]
async fn resume_briefing_session_only_no_signals_unlimited_iterations() {
    let h = WebStateHandle::new_test();
    let sid = "sxyz";
    h.push_activity_event(activity(sid, "progress", "2024-01-01T00:00:01Z"))
        .await;
    // Mission with max_iterations 0 → "∞", state Evaluating.
    let m = Mission {
        id: "m2".to_string(),
        goal: "g".to_string(),
        session_id: sid.to_string(),
        project_index: 0,
        state: MissionState::Evaluating,
        iteration: 1,
        max_iterations: 0,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    h.inner.write().await.missions.insert("m2".to_string(), m);

    let out = h
        .build_resume_briefing(ResumeBriefingRequest {
            active_session_id: Some(sid.to_string()),
            permissions: vec![],
            questions: vec![],
            signals: vec![],
        })
        .await
        .unwrap();
    assert!(out.summary.contains("evaluating, iteration 1/∞"));
    // No signals → no trailing bullet with signal part.
    assert!(!out.summary.ends_with('\u{2022}'));
}

#[tokio::test]
async fn resume_briefing_mission_only_no_session() {
    let h = WebStateHandle::new_test();
    // Empty active session → session_brief None; mission bound to empty session id.
    seed_mission(&h, "m3", "", MissionState::Paused).await;
    let out = h
        .build_resume_briefing(ResumeBriefingRequest {
            active_session_id: None,
            permissions: vec![],
            questions: vec![],
            signals: vec![],
        })
        .await
        .unwrap();
    assert_eq!(out.title, "Active mission");
    assert!(out.summary.contains("paused, iteration 2/5"));
    assert_eq!(out.next_action, "Check mission progress");
}

#[tokio::test]
async fn resume_briefing_signals_only() {
    let h = WebStateHandle::new_test();
    // No session brief (empty active id), no mission, but signals present.
    let out = h
        .build_resume_briefing(ResumeBriefingRequest {
            active_session_id: None,
            permissions: vec![],
            questions: vec![],
            signals: vec![signal("s1", "alpha"), signal("s2", "beta")],
        })
        .await
        .unwrap();
    assert_eq!(out.title, "Welcome back");
    assert_eq!(out.summary, "alpha \u{2022} beta");
    assert_eq!(out.next_action, "Check your recent signals");
}

// ── build_daily_summary ──────────────────────────────────────────────

#[tokio::test]
async fn daily_summary_unknown_routine_default_name() {
    let h = WebStateHandle::new_test();
    let out = h
        .build_daily_summary(DailySummaryRequest {
            routine_id: "does-not-exist".to_string(),
            permissions: vec![],
            questions: vec![],
            signals: vec![],
        })
        .await;
    assert_eq!(out, "Daily Summary: 0 active missions");
}

#[tokio::test]
async fn daily_summary_named_routine_with_attention_and_signals() {
    let h = WebStateHandle::new_test();
    let routine = h
        .create_routine(CreateRoutineRequest {
            name: "Morning".to_string(),
            trigger: RoutineTrigger::DailySummary,
            action: RoutineAction::SendMessage,
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
        })
        .await;
    seed_mission(&h, "m1", "s", MissionState::Executing).await;
    seed_mission(&h, "m2", "s", MissionState::Evaluating).await;

    let out = h
        .build_daily_summary(DailySummaryRequest {
            routine_id: routine.id.clone(),
            permissions: vec![perm("p", "s", "bash")],
            questions: vec![question("q", "s", "t")],
            signals: vec![
                signal("s1", "one"),
                signal("s2", "two"),
                signal("s3", "three"),
            ],
        })
        .await;
    assert!(out.starts_with("Morning: 2 active missions"));
    assert!(out.contains("2 items need attention"));
    assert!(out.contains("recent: one; two"));
    assert!(!out.contains("three"));
}
