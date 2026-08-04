use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

async fn seed_mission(h: &WebStateHandle, id: &str, state: MissionState) {
    let m = Mission {
        id: id.to_string(),
        goal: "g".to_string(),
        session_id: "s".to_string(),
        project_index: 0,
        state,
        iteration: 0,
        max_iterations: 5,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    h.inner.write().await.missions.insert(id.to_string(), m);
}

// ── build_assistant_stats ────────────────────────────────────────────

#[tokio::test]
async fn assistant_stats_counts_and_all_autonomy_modes() {
    let h = WebStateHandle::new_test();
    seed_mission(&h, "m1", MissionState::Executing).await;
    seed_mission(&h, "m2", MissionState::Evaluating).await;
    seed_mission(&h, "m3", MissionState::Paused).await;
    seed_mission(&h, "m4", MissionState::Completed).await;

    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "L".to_string(),
        content: "C".to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    })
    .await;
    h.create_delegated_work(CreateDelegatedWorkRequest {
        title: "t".to_string(),
        assignee: "a".to_string(),
        scope: "s".to_string(),
        mission_id: None,
        session_id: None,
        subagent_session_id: None,
    })
    .await;

    let req = || AssistantCenterStatsRequest {
        permissions: vec![PermissionInput {
            id: "p".to_string(),
            session_id: "s".to_string(),
            tool_name: "bash".to_string(),
            description: None,
            time: 1.0,
        }],
        questions: vec![],
    };

    // Default mode: observe.
    let stats = h.build_assistant_stats(req()).await;
    assert_eq!(stats.active_missions, 2); // Executing + Evaluating
    assert_eq!(stats.paused_missions, 1);
    assert_eq!(stats.total_missions, 4);
    assert_eq!(stats.pending_permissions, 1);
    assert_eq!(stats.pending_questions, 0);
    assert_eq!(stats.memory_items, 1);
    assert_eq!(stats.active_delegations, 1);
    assert_eq!(stats.autonomy_mode, "observe");

    h.update_autonomy_settings(AutonomyMode::Nudge).await;
    assert_eq!(h.build_assistant_stats(req()).await.autonomy_mode, "nudge");
    h.update_autonomy_settings(AutonomyMode::Continue).await;
    assert_eq!(
        h.build_assistant_stats(req()).await.autonomy_mode,
        "continue"
    );
    h.update_autonomy_settings(AutonomyMode::Autonomous).await;
    assert_eq!(
        h.build_assistant_stats(req()).await.autonomy_mode,
        "autonomous"
    );
}

#[tokio::test]
async fn assistant_stats_empty_state() {
    let h = WebStateHandle::new_test();
    let stats = h
        .build_assistant_stats(AssistantCenterStatsRequest {
            permissions: vec![],
            questions: vec![],
        })
        .await;
    assert_eq!(stats.active_missions, 0);
    assert_eq!(stats.total_missions, 0);
    assert_eq!(stats.workspace_count, 0);
    assert_eq!(stats.active_routines, 0);
}

// ── signals ──────────────────────────────────────────────────────────

#[tokio::test]
async fn signals_add_and_list_newest_first() {
    let h = WebStateHandle::new_test();
    assert!(h.list_signals().await.is_empty());
    let s1 = h
        .add_signal(AddSignalRequest {
            kind: "info".to_string(),
            title: "first".to_string(),
            body: "b1".to_string(),
            session_id: Some("sess".to_string()),
        })
        .await;
    assert!(s1.id.starts_with("signal-"));
    let s2 = h
        .add_signal(AddSignalRequest {
            kind: "warn".to_string(),
            title: "second".to_string(),
            body: "b2".to_string(),
            session_id: None,
        })
        .await;
    let all = h.list_signals().await;
    assert_eq!(all.len(), 2);
    // Newest inserted first.
    assert_eq!(all[0].id, s2.id);
    assert_eq!(all[1].id, s1.id);
    assert_eq!(all[0].title, "second");
    assert_eq!(all[1].session_id.as_deref(), Some("sess"));
}

#[tokio::test]
async fn signals_truncate_over_100() {
    let h = WebStateHandle::new_test();
    for i in 0..105 {
        h.add_signal(AddSignalRequest {
            kind: "k".to_string(),
            title: format!("t{i}"),
            body: "b".to_string(),
            session_id: None,
        })
        .await;
    }
    let all = h.list_signals().await;
    assert_eq!(all.len(), 100);
    // Newest (t104) is retained at the front.
    assert_eq!(all[0].title, "t104");
}

// ── workspace templates ──────────────────────────────────────────────

#[tokio::test]
async fn workspace_templates_builtins() {
    let templates = WebStateHandle::workspace_templates();
    assert_eq!(templates.len(), 4);
    let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["tpl-focus", "tpl-dev", "tpl-review", "tpl-morning"]
    );
    let focus = &templates[0];
    assert!(!focus.panels.sidebar);
    assert!(!focus.panels.terminal);
    let dev = &templates[1];
    assert!(dev.panels.terminal && dev.panels.editor && !dev.panels.git);
    let review = &templates[2];
    assert!(review.panels.git && !review.panels.terminal);
    let morning = &templates[3];
    assert!(
        morning.panels.sidebar
            && morning.panels.terminal
            && morning.panels.editor
            && morning.panels.git
    );
}

// ── list_active_memory ───────────────────────────────────────────────

#[tokio::test]
async fn active_memory_filters_by_scope() {
    let h = WebStateHandle::new_test();
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "global".to_string(),
        content: "c".to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    })
    .await;
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "proj".to_string(),
        content: "c".to_string(),
        scope: MemoryScope::Project,
        project_index: Some(3),
        session_id: None,
    })
    .await;
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "sess".to_string(),
        content: "c".to_string(),
        scope: MemoryScope::Session,
        project_index: None,
        session_id: Some("sid-1".to_string()),
    })
    .await;

    // No project / no session: only Global visible.
    let only_global = h.list_active_memory(None, None).await;
    assert_eq!(only_global.len(), 1);
    assert_eq!(only_global[0].label, "global");

    // Matching project index → Global + Project.
    let with_proj = h.list_active_memory(Some(3), None).await;
    let labels: Vec<&str> = with_proj.iter().map(|m| m.label.as_str()).collect();
    assert!(labels.contains(&"global"));
    assert!(labels.contains(&"proj"));
    assert!(!labels.contains(&"sess"));

    // Non-matching project index → Project filtered out.
    let wrong_proj = h.list_active_memory(Some(99), None).await;
    assert_eq!(wrong_proj.len(), 1);
    assert_eq!(wrong_proj[0].label, "global");

    // Matching session id → Global + Session.
    let with_sess = h.list_active_memory(None, Some("sid-1")).await;
    let labels: Vec<&str> = with_sess.iter().map(|m| m.label.as_str()).collect();
    assert!(labels.contains(&"global"));
    assert!(labels.contains(&"sess"));
    assert!(!labels.contains(&"proj"));

    // Non-matching session id → Session filtered out.
    let wrong_sess = h.list_active_memory(None, Some("other")).await;
    assert_eq!(wrong_sess.len(), 1);
    assert_eq!(wrong_sess[0].label, "global");
}
