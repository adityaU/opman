use super::*;
use serde_json::json;

#[test]
fn pipeline_stage_roundtrip_full() {
    let s = PipelineStage {
        lane_id: "lane-1".into(),
        session_id: Some("sess-9".into()),
        status: "running".into(),
        output: Some("hello".into()),
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["lane_id"], "lane-1");
    assert_eq!(v["session_id"], "sess-9");
    assert_eq!(v["status"], "running");
    assert_eq!(v["output"], "hello");
    let back: PipelineStage = serde_json::from_value(v).unwrap();
    assert_eq!(back.lane_id, "lane-1");
    assert_eq!(back.session_id.as_deref(), Some("sess-9"));
    assert_eq!(back.output.as_deref(), Some("hello"));
}

#[test]
fn pipeline_stage_defaults_when_absent() {
    let s: PipelineStage = serde_json::from_value(json!({
        "lane_id": "L",
        "status": "pending"
    }))
    .unwrap();
    assert_eq!(s.lane_id, "L");
    assert_eq!(s.status, "pending");
    assert!(s.session_id.is_none());
    assert!(s.output.is_none());
    // Clone + Debug coverage.
    let c = s.clone();
    assert!(format!("{c:?}").contains("PipelineStage"));
}

#[test]
fn pipeline_run_roundtrip_full() {
    let run = PipelineRun {
        task_id: "tsk_1".into(),
        stages: vec![PipelineStage {
            lane_id: "a".into(),
            session_id: None,
            status: "pending".into(),
            output: None,
        }],
        current_index: 2,
        status: "running".into(),
        launch_model: Some("opus".into()),
        launch_agent: Some("coder".into()),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-02T00:00:00Z".into(),
    };
    let v = serde_json::to_value(&run).unwrap();
    assert_eq!(v["task_id"], "tsk_1");
    assert_eq!(v["current_index"], 2);
    assert_eq!(v["launch_model"], "opus");
    assert_eq!(v["launch_agent"], "coder");
    let back: PipelineRun = serde_json::from_value(v).unwrap();
    assert_eq!(back.stages.len(), 1);
    assert_eq!(back.current_index, 2);
    let c = back.clone();
    assert!(format!("{c:?}").contains("PipelineRun"));
}

#[test]
fn pipeline_run_defaults_when_absent() {
    let run: PipelineRun = serde_json::from_value(json!({
        "task_id": "t",
        "stages": [],
        "current_index": 0,
        "status": "done",
        "created_at": "c",
        "updated_at": "u"
    }))
    .unwrap();
    assert!(run.launch_model.is_none());
    assert!(run.launch_agent.is_none());
    assert!(run.stages.is_empty());
}
