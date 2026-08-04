//! Generated coverage tests for `kanban_pipeline_brief.rs` (pure helpers).
use super::*;
use crate::web::types::default_board;

fn lane(id: &str, name: &str, terminal: bool, agent: Option<&str>, prompt: Option<&str>) -> Lane {
    Lane {
        id: id.into(),
        name: name.into(),
        color: "#fff".into(),
        wip: None,
        terminal,
        agent: agent.map(|s| s.into()),
        model: None,
        prompt: prompt.map(|s| s.into()),
    }
}

fn task(lane_id: &str, desc: &str) -> Task {
    Task {
        id: "tsk_1".into(),
        board_id: "brd".into(),
        lane_id: lane_id.into(),
        title: "My Task".into(),
        description: desc.into(),
        tags: vec![],
        priority: "high".into(),
        order_index: 1.0,
        session_id: None,
        launch_model: None,
        launch_agent: None,
        run_state: "idle".into(),
        archived: false,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[test]
fn stage_lanes_from_default_board_skips_terminal_and_plain() {
    let board = default_board("brd".into(), "/p".into());
    let stages = pipeline_stage_lanes(&board, "lane_todo");
    // planning, implementing, validating, codereview have agents; inreview is
    // terminal; todo and done have no agent/prompt.
    assert_eq!(
        stages,
        vec![
            "lane_planning".to_string(),
            "lane_implementing".to_string(),
            "lane_validating".to_string(),
            "lane_codereview".to_string(),
        ]
    );
}

#[test]
fn stage_lanes_unknown_current_lane_starts_from_zero() {
    let board = default_board("brd".into(), "/p".into());
    let from_zero = pipeline_stage_lanes(&board, "does_not_exist");
    let from_todo = pipeline_stage_lanes(&board, "lane_todo");
    assert_eq!(from_zero, from_todo);
}

#[test]
fn stage_lanes_prompt_only_lane_qualifies() {
    let board = Board {
        id: "b".into(),
        name: "B".into(),
        project_path: "/p".into(),
        lanes: vec![
            lane("l0", "Start", false, None, None),
            lane("l1", "PromptOnly", false, None, Some("do it")),
            lane("l2", "Term", true, None, None),
        ],
        transitions: Default::default(),
    };
    assert_eq!(pipeline_stage_lanes(&board, "l0"), vec!["l1".to_string()]);
}

#[test]
fn build_stage_brief_with_prompt_and_prev_output() {
    let t = task("l1", "Fix the bug");
    let l = lane(
        "l1",
        "Implement",
        false,
        Some("build"),
        Some("Write the code"),
    );
    let brief = build_stage_brief(&t, &l, 1, 3, Some("prior result"));
    assert!(brief.contains("STAGE 2/3: Implement"));
    assert!(brief.contains("PRIORITY: high"));
    assert!(brief.contains("Fix the bug"));
    assert!(brief.contains("Write the code"));
    assert!(brief.contains("OUTPUT FROM THE PREVIOUS STAGE"));
    assert!(brief.contains("prior result"));
    assert!(brief.contains("kanban_add_note(task_id=\"tsk_1\""));
}

#[test]
fn build_stage_brief_defaults_for_empty_desc_and_prompt() {
    let t = task("l1", "");
    let l = lane("l1", "Plan", false, None, Some("   "));
    let brief = build_stage_brief(&t, &l, 0, 1, None);
    assert!(brief.contains("(no description)"));
    assert!(brief.contains("Advance the task for this stage"));
    assert!(!brief.contains("OUTPUT FROM THE PREVIOUS STAGE"));
}

#[test]
fn inject_memory_empty_returns_brief_unchanged() {
    assert_eq!(inject_memory("hello", &[]), "hello");
}

#[test]
fn inject_memory_prepends_items() {
    let mem = vec![PersonalMemoryItem {
        id: "m".into(),
        label: "Rule".into(),
        content: "Be terse".into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }];
    let out = inject_memory("BRIEF", &mem);
    assert!(out.starts_with("[Assistant memory in effect]"));
    assert!(out.contains("- Rule: Be terse"));
    assert!(out.contains("[User request]\nBRIEF"));
}

#[test]
fn truncate_short_and_long_and_unicode() {
    assert_eq!(truncate("abc", 10), "abc");
    let out = truncate("abcdef", 3);
    assert!(out.starts_with("abc"));
    assert!(out.contains("[truncated]"));
    // Unicode counts chars, not bytes — 3 multibyte chars <= max 3 → unchanged.
    assert_eq!(truncate("héllo", 5), "héllo");
    assert!(truncate("héllo", 2).starts_with("hé"));
}
