//! Mock-upstream coverage for `web_state/kanban_pipeline.rs` SUCCESS paths.
//!
//! These drive `launch_kanban_pipeline`, `try_advance_kanban_pipeline`,
//! `start_stage` and `capture_session_output` against a tiny in-process axum
//! "opencode" server (via `start_mock_upstream` + `scope_base_url`) so the
//! create-session / dispatch-message / capture-output happy paths execute.
use super::*;
use crate::web::test_support::{scope_base_url, start_mock_upstream};
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
        description: "do the thing".into(),
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

/// A mock opencode server serving the three endpoints the pipeline touches:
/// POST `/session` (create → returns an id), GET `/session/{id}/message`
/// (capture output) and POST `/session/{id}/message` (dispatch the brief).
fn mock_upstream(created_id: &str, capture_text: &str) -> axum::Router {
    use axum::routing::{get, post};
    use axum::Json;
    let created = created_id.to_string();
    let text = capture_text.to_string();
    axum::Router::new()
        .route(
            "/session",
            post(move || {
                let created = created.clone();
                async move { Json(json!({ "id": created })) }
            }),
        )
        .route(
            "/session/{id}/message",
            get(move || {
                let text = text.clone();
                async move {
                    Json(json!([
                        { "info": { "role": "user", "time": { "created": 1 },
                            "parts": [{ "type": "text", "text": "prompt" }] } },
                        { "info": { "role": "assistant", "time": { "created": 9 },
                            "parts": [{ "type": "text", "text": text }] } }
                    ]))
                }
            })
            .post(|| async { Json(json!({ "ok": true })) }),
        )
}

// ── launch_kanban_pipeline: full success (stage moves lane, dispatches) ──

#[tokio::test]
async fn launch_success_starts_stage_zero() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    seed_task(&h, "brd", "tsk", "lane_todo");
    let base = start_mock_upstream(mock_upstream("sess-0", "")).await;
    let h2 = h.clone();
    let sid = scope_base_url(base, async move {
        h2.launch_kanban_pipeline("tsk", Some("sonnet".into()), Some("build".into()))
            .await
    })
    .await
    .expect("launch ok");
    assert_eq!(sid, "sess-0");

    let run = h.db_for_test().kanban_pipeline_get("tsk").unwrap();
    assert_eq!(run.status, "running");
    assert_eq!(run.current_index, 0);
    assert_eq!(run.stages[0].status, "running");
    assert_eq!(run.stages[0].session_id.as_deref(), Some("sess-0"));
    // Default board from lane_todo yields planning→implementing→validating→codereview.
    assert_eq!(run.stages.len(), 4);
    assert_eq!(run.launch_model.as_deref(), Some("sonnet"));

    let task = h.db_for_test().kanban_task("tsk").unwrap();
    assert_eq!(task.session_id.as_deref(), Some("sess-0"));
    assert_eq!(task.run_state, "running");
    assert_eq!(task.launch_agent.as_deref(), Some("build"));
    // Stage 0 lane differs from the starting lane → a transition note was written.
    assert_eq!(task.lane_id, "lane_planning");
    let notes = h.db_for_test().kanban_notes_for_task("tsk");
    assert!(notes.iter().any(|n| n.body.contains("Pipeline stage 1/4")));
}

#[tokio::test]
async fn launch_success_same_lane_writes_no_transition_note() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    // Task already in the first stage lane → start_stage's `from == lane_id`
    // branch runs and skips the transition note.
    seed_task(&h, "brd", "tsk", "lane_planning");
    let base = start_mock_upstream(mock_upstream("sess-x", "")).await;
    let h2 = h.clone();
    let sid = scope_base_url(base, async move {
        h2.launch_kanban_pipeline("tsk", None, None).await
    })
    .await
    .expect("launch ok");
    assert_eq!(sid, "sess-x");
    // No lane change ⇒ no note recorded at launch.
    assert!(h.db_for_test().kanban_notes_for_task("tsk").is_empty());
}

// ── try_advance_kanban_pipeline: middle stage chains the next one ───────

#[tokio::test]
async fn advance_middle_stage_success_chains_next() {
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
        launch_model: Some("opus".into()),
        launch_agent: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    });
    let base = start_mock_upstream(mock_upstream("sess-next", "planning summary")).await;
    let h2 = h.clone();
    scope_base_url(
        base,
        async move { h2.try_advance_kanban_pipeline("cur").await },
    )
    .await;

    let run = h.db_for_test().kanban_pipeline_get("tsk").unwrap();
    assert_eq!(run.status, "running");
    assert_eq!(run.current_index, 1);
    // Finished stage captured its assistant output.
    assert_eq!(run.stages[0].status, "done");
    assert_eq!(run.stages[0].output.as_deref(), Some("planning summary"));
    // Next stage was started with the new session.
    assert_eq!(run.stages[1].status, "running");
    assert_eq!(run.stages[1].session_id.as_deref(), Some("sess-next"));

    let task = h.db_for_test().kanban_task("tsk").unwrap();
    assert_eq!(task.session_id.as_deref(), Some("sess-next"));
    assert_eq!(task.run_state, "running");
    assert_eq!(task.lane_id, "lane_implementing");
}

// ── try_advance_kanban_pipeline: final stage completes with captured text ─

#[tokio::test]
async fn advance_final_stage_success_uses_captured_output_as_summary() {
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
    let mut rx = h.subscribe_events();
    let base = start_mock_upstream(mock_upstream("unused", "final review verdict")).await;
    let h2 = h.clone();
    scope_base_url(
        base,
        async move { h2.try_advance_kanban_pipeline("cur").await },
    )
    .await;

    let run = h.db_for_test().kanban_pipeline_get("tsk").unwrap();
    assert_eq!(run.status, "done");
    assert_eq!(
        run.stages[0].output.as_deref(),
        Some("final review verdict")
    );

    let task = h.db_for_test().kanban_task("tsk").unwrap();
    assert_eq!(task.run_state, "done");
    assert_eq!(task.lane_id, "lane_inreview"); // terminal lane
                                               // The captured output (not the default) became the summary note.
    let notes = h.db_for_test().kanban_notes_for_task("tsk");
    assert!(notes
        .iter()
        .any(|n| n.body.contains("final review verdict")));

    // A success Toast was broadcast.
    let mut saw_toast = false;
    while let Ok(ev) = rx.try_recv() {
        if let WebEvent::Toast { level, .. } = ev {
            if level == "success" {
                saw_toast = true;
            }
        }
    }
    assert!(saw_toast, "expected a pipeline-complete success Toast");
}
