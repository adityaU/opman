//! Generated coverage tests for `db/kanban_pipeline.rs` — pipeline run persistence.
use super::*;
use crate::web::types::default_board;

fn seed_task(db: &Db, board_id: &str, task_id: &str, project: &str) {
    let board = default_board(board_id.into(), project.into());
    db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");
    db.insert_kanban_task(&Task {
        id: task_id.into(),
        board_id: board_id.into(),
        lane_id: "lane_todo".into(),
        title: "t".into(),
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

fn run_with(
    task_id: &str,
    current: usize,
    status: &str,
    stages: Vec<PipelineStage>,
) -> PipelineRun {
    PipelineRun {
        task_id: task_id.into(),
        stages,
        current_index: current,
        status: status.into(),
        launch_model: Some("m".into()),
        launch_agent: Some("a".into()),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[test]
fn upsert_insert_then_update_and_get() {
    let db = Db::open_memory().unwrap();
    seed_task(&db, "brd", "tsk", "/p");

    assert!(db.kanban_pipeline_get("tsk").is_none());

    let run = run_with(
        "tsk",
        0,
        "running",
        vec![
            stage("lane_planning", Some("s0"), "running"),
            stage("lane_implementing", None, "pending"),
        ],
    );
    db.kanban_pipeline_upsert(&run);
    let got = db.kanban_pipeline_get("tsk").unwrap();
    assert_eq!(got.stages.len(), 2);
    assert_eq!(got.current_index, 0);
    assert_eq!(got.status, "running");
    assert_eq!(got.launch_model.as_deref(), Some("m"));

    // Conflict path: same task_id updates in place.
    let mut updated = got.clone();
    updated.current_index = 1;
    updated.status = "done".into();
    db.kanban_pipeline_upsert(&updated);
    let got2 = db.kanban_pipeline_get("tsk").unwrap();
    assert_eq!(got2.current_index, 1);
    assert_eq!(got2.status, "done");
}

#[test]
fn pipelines_for_board_joins_tasks() {
    let db = Db::open_memory().unwrap();
    seed_task(&db, "brdA", "t1", "/a");
    seed_task(&db, "brdB", "t2", "/b");
    db.kanban_pipeline_upsert(&run_with(
        "t1",
        0,
        "running",
        vec![stage("lane_planning", Some("x"), "running")],
    ));
    db.kanban_pipeline_upsert(&run_with(
        "t2",
        0,
        "done",
        vec![stage("lane_planning", Some("y"), "done")],
    ));

    let for_a = db.kanban_pipelines_for_board("brdA");
    assert_eq!(for_a.len(), 1);
    assert_eq!(for_a[0].task_id, "t1");
    assert!(db.kanban_pipelines_for_board("unknown").is_empty());
}

#[test]
fn by_session_matches_only_current_stage_of_running_run() {
    let db = Db::open_memory().unwrap();
    seed_task(&db, "brd", "tsk", "/p");
    // current_index = 1, and stage[1] owns session "live".
    db.kanban_pipeline_upsert(&run_with(
        "tsk",
        1,
        "running",
        vec![
            stage("lane_planning", Some("old"), "done"),
            stage("lane_implementing", Some("live"), "running"),
        ],
    ));

    // Matches the current stage's session.
    let found = db.kanban_pipeline_by_session("live").unwrap();
    assert_eq!(found.task_id, "tsk");

    // "old" appears in stages (LIKE hits) but is not the *current* stage → None.
    assert!(db.kanban_pipeline_by_session("old").is_none());

    // Session not present anywhere → None.
    assert!(db.kanban_pipeline_by_session("ghost").is_none());
}

#[test]
fn by_session_ignores_non_running_runs() {
    let db = Db::open_memory().unwrap();
    seed_task(&db, "brd", "tsk", "/p");
    db.kanban_pipeline_upsert(&run_with(
        "tsk",
        0,
        "stopped",
        vec![stage("lane_planning", Some("live"), "failed")],
    ));
    // status != 'running' → filtered out by the SQL WHERE clause.
    assert!(db.kanban_pipeline_by_session("live").is_none());
}
