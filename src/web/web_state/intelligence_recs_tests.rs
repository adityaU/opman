use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

fn perm(id: &str) -> PermissionInput {
    PermissionInput {
        id: id.to_string(),
        session_id: "sess".to_string(),
        tool_name: "bash".to_string(),
        description: None,
        time: 1.0,
    }
}

fn daily_summary_routine_req() -> CreateRoutineRequest {
    CreateRoutineRequest {
        name: "Daily".to_string(),
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
    }
}

fn memory_req() -> CreatePersonalMemoryRequest {
    CreatePersonalMemoryRequest {
        label: "L".to_string(),
        content: "C".to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    }
}

fn delegation_req(title: &str) -> CreateDelegatedWorkRequest {
    CreateDelegatedWorkRequest {
        title: title.to_string(),
        assignee: "agent".to_string(),
        scope: "scope".to_string(),
        mission_id: None,
        session_id: None,
        subagent_session_id: None,
    }
}

fn recipe_workspace() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        name: "recipe-ws".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        panels: WorkspacePanels { sidebar: true, terminal: true, editor: true, git: true },
        layout: WorkspaceLayout::default(),
        open_files: vec![],
        active_file: None,
        terminal_tabs: vec![],
        session_id: None,
        git_branch: None,
        is_template: false,
        recipe_description: Some("d".to_string()),
        recipe_next_action: None,
        is_recipe: true,
    }
}

#[tokio::test]
async fn recommendations_all_branches_fire_and_truncate_to_four() {
    let h = WebStateHandle::new_test();
    // Default: observe mode, no routine, no memory, no recipe workspace.
    // Add >2 incomplete delegations to trigger the overload branch.
    for i in 0..3 {
        h.create_delegated_work(delegation_req(&format!("d{i}"))).await;
    }
    let recs = h
        .build_recommendations(RecommendationsRequest {
            permissions: vec![perm("p1")],
            questions: vec![],
        })
        .await;
    // Many branches fire but the list is truncated to 4.
    assert_eq!(recs.len(), 4);
    // First is the daily copilot recommendation (observe + no daily summary).
    assert_eq!(recs[0].title, "Enable Daily Copilot");
    assert!(matches!(recs[0].action, RecommendationAction::SetupDailyCopilot));
    // IDs are sequential.
    assert_eq!(recs[0].id, "rec-1");
    assert_eq!(recs[1].id, "rec-2");
    assert!(matches!(recs[1].action, RecommendationAction::OpenInbox));
    assert!(recs[1].rationale.contains('1'));
}

#[tokio::test]
async fn recommendations_none_when_everything_satisfied() {
    let h = WebStateHandle::new_test();
    h.create_routine(daily_summary_routine_req()).await;
    h.create_personal_memory(memory_req()).await;
    h.update_autonomy_settings(AutonomyMode::Nudge).await;
    h.save_workspace(recipe_workspace()).await;
    let recs = h
        .build_recommendations(RecommendationsRequest {
            permissions: vec![],
            questions: vec![],
        })
        .await;
    assert!(recs.is_empty());
}

#[tokio::test]
async fn recommendations_delegation_not_overloaded() {
    let h = WebStateHandle::new_test();
    h.create_routine(daily_summary_routine_req()).await;
    h.create_personal_memory(memory_req()).await;
    h.update_autonomy_settings(AutonomyMode::Continue).await;
    h.save_workspace(recipe_workspace()).await;
    // Exactly 2 incomplete → not > 2, so no overload rec.
    h.create_delegated_work(delegation_req("a")).await;
    h.create_delegated_work(delegation_req("b")).await;
    let recs = h
        .build_recommendations(RecommendationsRequest {
            permissions: vec![],
            questions: vec![],
        })
        .await;
    assert!(recs.is_empty());
}

#[tokio::test]
async fn recommendations_question_only_blocker_and_memory_present() {
    let h = WebStateHandle::new_test();
    // Add daily-summary routine + memory + recipe so only the blocker branch
    // (from a question) and the observe-derived branches vary.
    h.create_routine(daily_summary_routine_req()).await;
    h.create_personal_memory(memory_req()).await;
    h.save_workspace(recipe_workspace()).await;
    let recs = h
        .build_recommendations(RecommendationsRequest {
            permissions: vec![],
            questions: vec![QuestionInput {
                id: "q".to_string(),
                session_id: "s".to_string(),
                title: "t".to_string(),
                time: 1.0,
            }],
        })
        .await;
    // Blocker rec (high) + observe-upgrade rec (low). No daily-copilot because a
    // daily summary routine exists.
    let titles: Vec<&str> = recs.iter().map(|r| r.title.as_str()).collect();
    assert!(titles.contains(&"Clear assistant blockers"));
    assert!(titles.contains(&"Enable more proactive assistance"));
    assert!(!titles.contains(&"Teach your assistant"));
}
