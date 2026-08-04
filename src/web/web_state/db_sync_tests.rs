use super::*;
use crate::web::db::Db;
use crate::web::types::*;

fn mission() -> Mission {
    Mission {
        id: "m1".into(),
        goal: "ship it".into(),
        session_id: "s1".into(),
        project_index: 2,
        state: MissionState::Executing,
        iteration: 3,
        max_iterations: 10,
        last_verdict: Some(EvalVerdict::Continue),
        last_eval_summary: Some("progress".into()),
        eval_history: vec![EvalRecord {
            iteration: 1,
            verdict: EvalVerdict::Continue,
            summary: "step".into(),
            next_step: Some("more".into()),
            timestamp: "2024-01-01T00:00:00Z".into(),
        }],
        created_at: "2024-01-01T00:00:00Z".into(),
        updated_at: "2024-01-02T00:00:00Z".into(),
    }
}

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
        action: RoutineAction::SendMessage,
        enabled: true,
        cron_expr: Some("0 9 * * *".into()),
        timezone: Some("UTC".into()),
        target_mode: Some(RoutineTargetMode::ExistingSession),
        session_id: Some("s1".into()),
        project_index: Some(0),
        prompt: Some("hi".into()),
        provider_id: Some("anthropic".into()),
        model_id: Some("claude".into()),
        mission_id: Some("m1".into()),
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

fn delegated() -> DelegatedWorkItem {
    DelegatedWorkItem {
        id: "d1".into(),
        title: "task".into(),
        assignee: "bob".into(),
        scope: "web".into(),
        status: DelegationStatus::Running,
        mission_id: Some("m1".into()),
        session_id: Some("s1".into()),
        subagent_session_id: Some("s2".into()),
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn workspace() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        name: "ws1".into(),
        created_at: "t".into(),
        panels: WorkspacePanels {
            sidebar: true,
            terminal: false,
            editor: true,
            git: false,
        },
        layout: WorkspaceLayout::default(),
        open_files: vec!["a.rs".into()],
        active_file: Some("a.rs".into()),
        terminal_tabs: vec![WorkspaceTerminalTab {
            label: "sh".into(),
            kind: "shell".into(),
        }],
        session_id: Some("s1".into()),
        git_branch: Some("main".into()),
        is_template: false,
        recipe_description: None,
        recipe_next_action: None,
        is_recipe: false,
    }
}

fn signal() -> SignalInput {
    SignalInput {
        id: "sig1".into(),
        kind: "info".into(),
        title: "hi".into(),
        body: "body".into(),
        created_at: 1.0,
        session_id: Some("s1".into()),
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
    let missions = vec![mission()];
    let memory = vec![memory_item()];
    let auto = autonomy();
    let routines = vec![routine()];
    let runs = vec![run_record()];
    let deleg = vec![delegated()];
    let ws = vec![workspace()];
    let signals = vec![signal()];

    super::sync_all(
        &db, &missions, &memory, &auto, &routines, &runs, &deleg, &ws, &signals,
    )
    .expect("sync ok");

    assert_eq!(db.list_missions().len(), 1);
    assert_eq!(db.list_missions()[0].goal, "ship it");
    assert_eq!(db.list_memory().len(), 1);
    assert_eq!(db.list_routines().len(), 1);
    assert_eq!(db.list_routine_runs().len(), 1);
    assert_eq!(db.list_delegated_work().len(), 1);
    assert_eq!(db.list_workspaces().len(), 1);
    assert_eq!(db.list_signals(100).len(), 1);
    assert!(matches!(
        db.load_autonomy_settings().mode,
        AutonomyMode::Nudge
    ));
}

#[test]
fn sync_all_replaces_previous_rows() {
    let db = Db::open_memory().expect("mem db");
    let auto = autonomy();
    // First write with one mission.
    super::sync_all(&db, &[mission()], &[], &auto, &[], &[], &[], &[], &[]).unwrap();
    assert_eq!(db.list_missions().len(), 1);
    // Second write with empty slices clears everything (DELETE then re-insert).
    super::sync_all(&db, &[], &[], &auto, &[], &[], &[], &[], &[]).unwrap();
    assert_eq!(db.list_missions().len(), 0);
}

#[test]
fn sync_all_empty_is_ok() {
    let db = Db::open_memory().expect("mem db");
    let auto = autonomy();
    super::sync_all(&db, &[], &[], &auto, &[], &[], &[], &[], &[]).expect("empty ok");
    assert_eq!(db.list_signals(10).len(), 0);
}

// ── String conversion helpers: every variant ────────────────────────

#[test]
fn state_str_all_variants() {
    use super::state_str;
    assert_eq!(state_str(&MissionState::Pending), "pending");
    assert_eq!(state_str(&MissionState::Executing), "executing");
    assert_eq!(state_str(&MissionState::Evaluating), "evaluating");
    assert_eq!(state_str(&MissionState::Paused), "paused");
    assert_eq!(state_str(&MissionState::Completed), "completed");
    assert_eq!(state_str(&MissionState::Cancelled), "cancelled");
    assert_eq!(state_str(&MissionState::Failed), "failed");
}

#[test]
fn verdict_str_all_variants() {
    use super::verdict_str;
    assert_eq!(verdict_str(&EvalVerdict::Achieved), "achieved");
    assert_eq!(verdict_str(&EvalVerdict::Continue), "continue");
    assert_eq!(verdict_str(&EvalVerdict::Blocked), "blocked");
    assert_eq!(verdict_str(&EvalVerdict::Failed), "failed");
}

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
fn action_str_all_variants() {
    use super::action_str;
    assert_eq!(action_str(&RoutineAction::SendMessage), "send_message");
    assert_eq!(action_str(&RoutineAction::ReviewMission), "review_mission");
    assert_eq!(action_str(&RoutineAction::OpenInbox), "open_inbox");
    assert_eq!(
        action_str(&RoutineAction::OpenActivityFeed),
        "open_activity_feed"
    );
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

#[test]
fn deleg_str_all_variants() {
    use super::deleg_str;
    assert_eq!(deleg_str(&DelegationStatus::Planned), "planned");
    assert_eq!(deleg_str(&DelegationStatus::Running), "running");
    assert_eq!(deleg_str(&DelegationStatus::Completed), "completed");
}
