//! Generated coverage tests for `web_state/kanban_pipeline.rs`.
use super::*;
use crate::web::types::default_board;
use crate::web::web_state::WebStateHandle;
use serde_json::json;

fn seed_board(h: &WebStateHandle, board_id: &str, project: &str) {
    h.db_for_test().insert_kanban_board(
        &default_board(board_id.into(), project.into()),
        "2026-01-01T00:00:00Z",
    );
}

fn seed_task(h: &WebStateHandle, board_id: &str, task_id: &str, lane: &str) {
    h.db_for_test().insert_kanban_task(&Task {
        id: task_id.into(),
        board_id: board_id.into(),
        lane_id: lane.into(),
        title: "T".into(),
        description: String::new(),
        tags: vec![],
        priority: "normal".into(),
        order_index: 1.0,
        session_id: None,
        launch_model: None,
        launch_agent: None,
        run_state: "idle".into(),
        archived: false,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    });
}

fn stage(lane: &str, session: Option<&str>, status: &str) -> PipelineStage {
    PipelineStage {
        lane_id: lane.into(),
        session_id: session.map(|s| s.into()),
        status: status.into(),
        output: None,
    }
}

// ── launch_kanban_pipeline ──────────────────────────────────────────

#[tokio::test]
async fn launch_task_not_found() {
    let h = WebStateHandle::new_test();
    assert_eq!(
        h.launch_kanban_pipeline("ghost", None, None)
            .await
            .unwrap_err(),
        "task not found"
    );
}

#[tokio::test]
async fn launch_no_stages_errors() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    // lane_done has no agent/prompt and nothing after it qualifies.
    seed_task(&h, "brd", "tsk", "lane_done");
    let err = h
        .launch_kanban_pipeline("tsk", None, None)
        .await
        .unwrap_err();
    assert!(err.contains("No pipeline stages"));
}

#[tokio::test]
async fn launch_start_stage_http_failure_errors() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    seed_task(&h, "brd", "tsk", "lane_todo");
    // Stages exist; start_stage attempts an HTTP call to the (absent) upstream
    // and fails fast → launch returns the propagated error.
    let err = h
        .launch_kanban_pipeline("tsk", Some("m".into()), Some("build".into()))
        .await
        .unwrap_err();
    assert!(
        err.contains("create session failed") || err.contains("failed"),
        "got: {err}"
    );
    // A stage-transition note was recorded before the HTTP attempt.
    assert!(!h.db_for_test().kanban_notes_for_task("tsk").is_empty());
}

// ── stop_kanban_pipeline ────────────────────────────────────────────

#[tokio::test]
async fn stop_no_run_is_noop() {
    let h = WebStateHandle::new_test();
    h.stop_kanban_pipeline("tsk").await; // must not panic
}

#[tokio::test]
async fn stop_non_running_run_is_noop() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    seed_task(&h, "brd", "tsk", "lane_todo");
    h.db_for_test().kanban_pipeline_upsert(&PipelineRun {
        task_id: "tsk".into(),
        stages: vec![stage("lane_planning", Some("s"), "done")],
        current_index: 0,
        status: "done".into(),
        launch_model: None,
        launch_agent: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    });
    h.stop_kanban_pipeline("tsk").await;
    assert_eq!(
        h.db_for_test().kanban_pipeline_get("tsk").unwrap().status,
        "done"
    );
}

#[tokio::test]
async fn stop_running_run_marks_stopped_and_stage_failed() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    seed_task(&h, "brd", "tsk", "lane_todo");
    h.db_for_test().kanban_pipeline_upsert(&PipelineRun {
        task_id: "tsk".into(),
        stages: vec![stage("lane_planning", Some("s"), "running")],
        current_index: 0,
        status: "running".into(),
        launch_model: None,
        launch_agent: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    });
    h.stop_kanban_pipeline("tsk").await;
    let run = h.db_for_test().kanban_pipeline_get("tsk").unwrap();
    assert_eq!(run.status, "stopped");
    assert_eq!(run.stages[0].status, "failed");
}

// ── try_advance_kanban_pipeline ─────────────────────────────────────

#[tokio::test]
async fn advance_no_run_for_session_is_noop() {
    let h = WebStateHandle::new_test();
    h.try_advance_kanban_pipeline("unknown-session").await; // must not panic
}

#[tokio::test]
async fn advance_middle_stage_http_failure_marks_failed() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    seed_task(&h, "brd", "tsk", "lane_planning");
    h.db_for_test().kanban_pipeline_upsert(&PipelineRun {
        task_id: "tsk".into(),
        stages: vec![
            stage("lane_planning", Some("cur"), "running"),
            stage("lane_implementing", None, "pending"),
        ],
        current_index: 0,
        status: "running".into(),
        launch_model: None,
        launch_agent: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    });
    h.try_advance_kanban_pipeline("cur").await;
    let run = h.db_for_test().kanban_pipeline_get("tsk").unwrap();
    // Stage 0 marked done; chaining stage 1 failed on HTTP → run failed.
    assert_eq!(run.stages[0].status, "done");
    assert_eq!(run.status, "failed");
    let task = h.db_for_test().kanban_task("tsk").unwrap();
    assert_eq!(task.run_state, "failed");
    let notes = h.db_for_test().kanban_notes_for_task("tsk");
    assert!(notes.iter().any(|n| n.body.contains("Pipeline stalled")));
}

#[tokio::test]
async fn advance_final_stage_completes_to_terminal() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    seed_task(&h, "brd", "tsk", "lane_codereview");
    h.db_for_test().kanban_pipeline_upsert(&PipelineRun {
        task_id: "tsk".into(),
        stages: vec![stage("lane_codereview", Some("cur"), "running")],
        current_index: 0,
        status: "running".into(),
        launch_model: None,
        launch_agent: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    });
    h.try_advance_kanban_pipeline("cur").await;
    let run = h.db_for_test().kanban_pipeline_get("tsk").unwrap();
    assert_eq!(run.status, "done");
    let task = h.db_for_test().kanban_task("tsk").unwrap();
    assert_eq!(task.run_state, "done");
    assert_eq!(task.lane_id, "lane_inreview"); // terminal lane
    let notes = h.db_for_test().kanban_notes_for_task("tsk");
    assert!(notes.iter().any(|n| n.body.contains("ready for review")));
}

// ── latest_assistant_output (pure helper) ───────────────────────────

fn msg(role: &str, created: u64, text: &str) -> serde_json::Value {
    json!({
        "info": {
            "role": role,
            "time": { "created": created },
            "parts": [{ "type": "text", "text": text }]
        }
    })
}

#[test]
fn latest_output_from_array_picks_newest_assistant() {
    let body = json!([
        msg("user", 1, "hi"),
        msg("assistant", 2, "older"),
        msg("assistant", 5, "newest"),
    ]);
    assert_eq!(latest_assistant_output(&body).as_deref(), Some("newest"));
}

#[test]
fn latest_output_from_object_map() {
    let body = json!({ "a": msg("assistant", 3, "only") });
    assert_eq!(latest_assistant_output(&body).as_deref(), Some("only"));
}

#[test]
fn latest_output_none_cases() {
    // Not an array or object.
    assert!(latest_assistant_output(&json!("scalar")).is_none());
    // No assistant messages.
    assert!(latest_assistant_output(&json!([msg("user", 1, "x")])).is_none());
    // Assistant message but empty text → filtered out.
    assert!(latest_assistant_output(&json!([msg("assistant", 1, "   ")])).is_none());
}
