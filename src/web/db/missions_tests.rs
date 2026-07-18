//! Generated coverage tests for `db/missions.rs`: update-row, ordering, and all
//! mission-state / eval-verdict string<->enum conversions (incl. legacy states).
use super::*;

fn base_mission(id: &str, updated: &str) -> Mission {
    Mission {
        id: id.into(),
        goal: format!("g-{id}"),
        session_id: "s".into(),
        project_index: 0,
        state: MissionState::Pending,
        iteration: 0,
        max_iterations: 10,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: updated.into(),
    }
}

#[test]
fn list_sorted_by_updated_desc() {
    let db = Db::open_memory().unwrap();
    db.insert_mission(&base_mission("a", "2025-01-01T00:00:00Z"));
    db.insert_mission(&base_mission("b", "2025-05-01T00:00:00Z"));
    let list = db.list_missions();
    assert_eq!(list[0].id, "b");
    assert_eq!(list[1].id, "a");
}

#[test]
fn update_mission_row_found_and_not_found() {
    let db = Db::open_memory().unwrap();
    let mut m = base_mission("u1", "2025-01-01T00:00:00Z");
    db.insert_mission(&m);

    m.goal = "updated goal".into();
    m.state = MissionState::Completed;
    m.iteration = 4;
    m.last_verdict = Some(EvalVerdict::Achieved);
    m.last_eval_summary = Some("done".into());
    m.eval_history = vec![EvalRecord {
        iteration: 1,
        verdict: EvalVerdict::Blocked,
        summary: "stuck".into(),
        next_step: None,
        timestamp: "2025-01-01T00:00:00Z".into(),
    }];
    m.updated_at = "2025-02-01T00:00:00Z".into();
    assert!(db.update_mission_row(&m));

    let got = &db.list_missions()[0];
    assert_eq!(got.goal, "updated goal");
    assert_eq!(got.state, MissionState::Completed);
    assert_eq!(got.iteration, 4);
    assert!(matches!(got.last_verdict, Some(EvalVerdict::Achieved)));
    assert_eq!(got.eval_history.len(), 1);

    assert!(!db.update_mission_row(&base_mission("ghost", "2025-01-01T00:00:00Z")));
}

#[test]
fn delete_mission_row_missing_is_false() {
    let db = Db::open_memory().unwrap();
    assert!(!db.delete_mission_row("nope"));
}

#[test]
fn mission_state_conversions_roundtrip() {
    for s in [
        MissionState::Pending,
        MissionState::Executing,
        MissionState::Evaluating,
        MissionState::Paused,
        MissionState::Completed,
        MissionState::Cancelled,
        MissionState::Failed,
    ] {
        assert_eq!(parse_mission_state(mission_state_str(&s)), s);
    }
}

#[test]
fn mission_state_legacy_and_unknown_map_to_pending() {
    for legacy in ["planned", "active", "blocked", "garbage"] {
        assert_eq!(parse_mission_state(legacy), MissionState::Pending);
    }
}

#[test]
fn eval_verdict_conversions_roundtrip_and_unknown() {
    for v in [
        EvalVerdict::Achieved,
        EvalVerdict::Continue,
        EvalVerdict::Blocked,
        EvalVerdict::Failed,
    ] {
        let parsed = parse_eval_verdict(eval_verdict_str(&v)).unwrap();
        // EvalVerdict has no PartialEq; compare via its string form.
        assert_eq!(eval_verdict_str(&parsed), eval_verdict_str(&v));
    }
    assert!(parse_eval_verdict("unknown").is_none());
}

#[test]
fn mission_with_verdict_and_history_persists_through_list() {
    let db = Db::open_memory().unwrap();
    let mut m = base_mission("m", "2025-01-01T00:00:00Z");
    m.last_verdict = Some(EvalVerdict::Continue);
    db.insert_mission(&m);
    let got = &db.list_missions()[0];
    assert!(matches!(got.last_verdict, Some(EvalVerdict::Continue)));
}
