use super::*;
use serde_json::json;

#[test]
fn mission_state_snake_case_roundtrip() {
    let cases = [
        (MissionState::Pending, "pending"),
        (MissionState::Executing, "executing"),
        (MissionState::Evaluating, "evaluating"),
        (MissionState::Paused, "paused"),
        (MissionState::Completed, "completed"),
        (MissionState::Cancelled, "cancelled"),
        (MissionState::Failed, "failed"),
    ];
    for (state, name) in cases {
        assert_eq!(serde_json::to_value(state.clone()).unwrap(), name);
        let back: MissionState = serde_json::from_value(json!(name)).unwrap();
        assert_eq!(back, state);
    }
}

#[test]
fn mission_state_eq_and_debug() {
    assert_eq!(MissionState::Pending, MissionState::Pending);
    assert_ne!(MissionState::Pending, MissionState::Failed);
    assert!(format!("{:?}", MissionState::Executing).contains("Executing"));
}

#[test]
fn eval_verdict_snake_case_roundtrip() {
    let cases = [
        (EvalVerdict::Achieved, "achieved"),
        (EvalVerdict::Continue, "continue"),
        (EvalVerdict::Blocked, "blocked"),
        (EvalVerdict::Failed, "failed"),
    ];
    for (verdict, name) in cases {
        assert_eq!(serde_json::to_value(verdict.clone()).unwrap(), name);
        let back: EvalVerdict = serde_json::from_value(json!(name)).unwrap();
        assert!(matches!(
            (back, name),
            (EvalVerdict::Achieved, "achieved")
                | (EvalVerdict::Continue, "continue")
                | (EvalVerdict::Blocked, "blocked")
                | (EvalVerdict::Failed, "failed")
        ));
    }
}

#[test]
fn eval_record_roundtrip_and_skip() {
    let rec = EvalRecord {
        iteration: 2,
        verdict: EvalVerdict::Continue,
        summary: "progress".into(),
        next_step: Some("do X".into()),
        timestamp: "2026-01-01T00:00:00Z".into(),
    };
    let v = serde_json::to_value(&rec).unwrap();
    assert_eq!(v["iteration"], 2);
    assert_eq!(v["verdict"], "continue");
    assert_eq!(v["next_step"], "do X");
    let back: EvalRecord = serde_json::from_value(v).unwrap();
    assert_eq!(back.iteration, 2);
    assert!(format!("{:?}", back.clone()).contains("EvalRecord"));

    // next_step None is skipped when serializing.
    let rec2 = EvalRecord {
        iteration: 1,
        verdict: EvalVerdict::Achieved,
        summary: "done".into(),
        next_step: None,
        timestamp: "t".into(),
    };
    let v2 = serde_json::to_value(&rec2).unwrap();
    assert!(v2.get("next_step").is_none());
    // Also deserializable when next_step absent (serde default).
    let back2: EvalRecord = serde_json::from_value(v2).unwrap();
    assert!(back2.next_step.is_none());
}

#[test]
fn mission_roundtrip_full() {
    let m = Mission {
        id: "mission1".into(),
        goal: "ship it".into(),
        session_id: "s1".into(),
        project_index: 4,
        state: MissionState::Executing,
        iteration: 3,
        max_iterations: 10,
        last_verdict: Some(EvalVerdict::Continue),
        last_eval_summary: Some("working".into()),
        eval_history: vec![EvalRecord {
            iteration: 1,
            verdict: EvalVerdict::Continue,
            summary: "s".into(),
            next_step: None,
            timestamp: "t".into(),
        }],
        created_at: "c".into(),
        updated_at: "u".into(),
    };
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["id"], "mission1");
    assert_eq!(v["state"], "executing");
    assert_eq!(v["last_verdict"], "continue");
    assert_eq!(v["last_eval_summary"], "working");
    assert_eq!(v["eval_history"].as_array().unwrap().len(), 1);
    let back: Mission = serde_json::from_value(v).unwrap();
    assert_eq!(back.project_index, 4);
    assert_eq!(back.iteration, 3);
    assert!(format!("{:?}", back.clone()).contains("Mission"));
}

#[test]
fn mission_skips_empty_optionals() {
    let m = Mission {
        id: "m".into(),
        goal: "g".into(),
        session_id: "s".into(),
        project_index: 0,
        state: MissionState::Pending,
        iteration: 0,
        max_iterations: 0,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "c".into(),
        updated_at: "u".into(),
    };
    let v = serde_json::to_value(&m).unwrap();
    assert!(v.get("last_verdict").is_none());
    assert!(v.get("last_eval_summary").is_none());
    assert!(v.get("eval_history").is_none()); // skip_serializing_if Vec::is_empty
                                               // Deserialize back with those absent.
    let back: Mission = serde_json::from_value(v).unwrap();
    assert!(back.last_verdict.is_none());
    assert!(back.eval_history.is_empty());
}

#[test]
fn create_mission_request_full_and_defaults() {
    let full: CreateMissionRequest = serde_json::from_value(json!({
        "goal": "g",
        "session_id": "s",
        "project_index": 2,
        "max_iterations": 5
    }))
    .unwrap();
    assert_eq!(full.goal, "g");
    assert_eq!(full.session_id.as_deref(), Some("s"));
    assert_eq!(full.project_index, Some(2));
    assert_eq!(full.max_iterations, Some(5));

    let minimal: CreateMissionRequest = serde_json::from_value(json!({"goal": "g"})).unwrap();
    assert!(minimal.session_id.is_none());
    assert!(minimal.project_index.is_none());
    assert!(minimal.max_iterations.is_none());
    assert!(format!("{:?}", minimal.clone()).contains("CreateMissionRequest"));
}

#[test]
fn update_mission_request_defaults() {
    let empty: UpdateMissionRequest = serde_json::from_value(json!({})).unwrap();
    assert!(empty.goal.is_none());
    assert!(empty.max_iterations.is_none());
    let set: UpdateMissionRequest =
        serde_json::from_value(json!({"goal": "x", "max_iterations": 9})).unwrap();
    assert_eq!(set.goal.as_deref(), Some("x"));
    assert_eq!(set.max_iterations, Some(9));
    assert!(format!("{:?}", set.clone()).contains("UpdateMissionRequest"));
}

#[test]
fn mission_action_and_request() {
    for (name, is_variant) in [
        ("start", 0),
        ("pause", 1),
        ("resume", 2),
        ("cancel", 3),
    ] {
        let a: MissionAction = serde_json::from_value(json!(name)).unwrap();
        let matched = matches!(
            (a, is_variant),
            (MissionAction::Start, 0)
                | (MissionAction::Pause, 1)
                | (MissionAction::Resume, 2)
                | (MissionAction::Cancel, 3)
        );
        assert!(matched);
    }
    let req: MissionActionRequest = serde_json::from_value(json!({"action": "pause"})).unwrap();
    assert!(matches!(req.action, MissionAction::Pause));
    assert!(format!("{:?}", req.clone()).contains("MissionActionRequest"));
    assert!(serde_json::from_value::<MissionAction>(json!("nope")).is_err());
}

#[test]
fn missions_list_response_serialize() {
    let resp = MissionsListResponse { missions: vec![] };
    let v = serde_json::to_value(&resp).unwrap();
    assert!(v["missions"].as_array().unwrap().is_empty());
    assert!(format!("{:?}", resp.clone()).contains("MissionsListResponse"));
}
