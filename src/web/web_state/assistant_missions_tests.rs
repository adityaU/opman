//! Generated tests for assistant.rs — part 1: missions CRUD, mission_action,
//! private prompt/send helpers, and the pure eval-parsing helpers.
use super::*;
use crate::web::web_state::WebStateHandle;

// Set BASE_URL to a fail-fast loopback address so any code path that reaches
// `crate::app::base_url()` gets an immediate connection-refused instead of a
// panic or a real network call. Safe to call repeatedly (OnceLock::set errors
// after the first set, which we ignore).
fn ensure_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

fn mk_mission(id: &str, session: &str, state: MissionState, iteration: u32, max: u32) -> Mission {
    let now = Utc::now().to_rfc3339();
    Mission {
        id: id.to_string(),
        goal: "reach the goal".to_string(),
        session_id: session.to_string(),
        project_index: 0,
        state,
        iteration,
        max_iterations: max,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

async fn insert_mission(h: &WebStateHandle, m: Mission) {
    let mut s = h.inner.write().await;
    s.missions.insert(m.id.clone(), m);
}

// ── CRUD ────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_missions_empty() {
    let h = WebStateHandle::new_test();
    assert!(h.list_missions().await.is_empty());
}

#[tokio::test]
async fn list_missions_sorted_by_updated_desc() {
    let h = WebStateHandle::new_test();
    let mut a = mk_mission("m-a", "", MissionState::Pending, 0, 10);
    a.updated_at = "2020-01-01T00:00:00Z".to_string();
    let mut b = mk_mission("m-b", "", MissionState::Pending, 0, 10);
    b.updated_at = "2021-01-01T00:00:00Z".to_string();
    insert_mission(&h, a).await;
    insert_mission(&h, b).await;
    let list = h.list_missions().await;
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "m-b"); // newer first
    assert_eq!(list[1].id, "m-a");
}

#[tokio::test]
async fn get_mission_some_and_none() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Pending, 0, 10)).await;
    assert!(h.get_mission("m1").await.is_some());
    assert!(h.get_mission("nope").await.is_none());
}

#[tokio::test]
async fn create_mission_defaults() {
    let h = WebStateHandle::new_test();
    let req = CreateMissionRequest {
        goal: "do it".to_string(),
        session_id: None,
        project_index: None,
        max_iterations: None,
    };
    let m = h.create_mission(req).await;
    assert_eq!(m.goal, "do it");
    assert_eq!(m.session_id, ""); // default empty
    assert_eq!(m.project_index, 0); // active_project default
    assert_eq!(m.max_iterations, 10); // default
    assert_eq!(m.iteration, 0);
    assert_eq!(m.state, MissionState::Pending);
    assert!(m.id.starts_with("mission-"));
    // persisted in state
    assert!(h.get_mission(&m.id).await.is_some());
}

#[tokio::test]
async fn create_mission_explicit_values() {
    let h = WebStateHandle::new_test();
    let req = CreateMissionRequest {
        goal: "g".to_string(),
        session_id: Some("sess-1".to_string()),
        project_index: Some(3),
        max_iterations: Some(0),
    };
    let m = h.create_mission(req).await;
    assert_eq!(m.session_id, "sess-1");
    assert_eq!(m.project_index, 3);
    assert_eq!(m.max_iterations, 0);
}

#[tokio::test]
async fn update_mission_not_found() {
    let h = WebStateHandle::new_test();
    let req = UpdateMissionRequest {
        goal: Some("x".into()),
        max_iterations: None,
    };
    assert!(h.update_mission("missing", req).await.is_none());
}

#[tokio::test]
async fn update_mission_goal_and_iterations() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Pending, 0, 10)).await;
    let req = UpdateMissionRequest {
        goal: Some("new goal".into()),
        max_iterations: Some(42),
    };
    let updated = h.update_mission("m1", req).await.unwrap();
    assert_eq!(updated.goal, "new goal");
    assert_eq!(updated.max_iterations, 42);
}

#[tokio::test]
async fn update_mission_no_fields() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Pending, 0, 10)).await;
    let req = UpdateMissionRequest {
        goal: None,
        max_iterations: None,
    };
    let updated = h.update_mission("m1", req).await.unwrap();
    assert_eq!(updated.goal, "reach the goal"); // unchanged
    assert_eq!(updated.max_iterations, 10);
}

#[tokio::test]
async fn delete_mission_found_and_missing() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Pending, 0, 10)).await;
    assert!(h.delete_mission("m1").await);
    assert!(!h.delete_mission("m1").await); // already gone
    assert!(!h.delete_mission("never").await);
}

// ── mission_action ──────────────────────────────────────────────────

#[tokio::test]
async fn mission_action_not_found() {
    let h = WebStateHandle::new_test();
    let r = h.mission_action("missing", MissionAction::Start).await;
    assert_eq!(r.unwrap_err(), "Mission not found");
}

#[tokio::test]
async fn mission_action_start_from_pending() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    // empty session_id → kick_mission_execution early-returns (warn), no network
    insert_mission(&h, mk_mission("m1", "", MissionState::Pending, 0, 10)).await;
    let m = h.mission_action("m1", MissionAction::Start).await.unwrap();
    assert_eq!(m.state, MissionState::Executing);
    assert_eq!(m.iteration, 1);
}

#[tokio::test]
async fn mission_action_start_invalid_state() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Executing, 1, 10)).await;
    let err = h
        .mission_action("m1", MissionAction::Start)
        .await
        .unwrap_err();
    assert!(err.contains("Cannot start"));
}

#[tokio::test]
async fn mission_action_pause_from_executing_and_evaluating() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Executing, 1, 10)).await;
    insert_mission(&h, mk_mission("m2", "", MissionState::Evaluating, 1, 10)).await;
    assert_eq!(
        h.mission_action("m1", MissionAction::Pause)
            .await
            .unwrap()
            .state,
        MissionState::Paused
    );
    assert_eq!(
        h.mission_action("m2", MissionAction::Pause)
            .await
            .unwrap()
            .state,
        MissionState::Paused
    );
}

#[tokio::test]
async fn mission_action_pause_invalid() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Pending, 0, 10)).await;
    let err = h
        .mission_action("m1", MissionAction::Pause)
        .await
        .unwrap_err();
    assert!(err.contains("Cannot pause"));
}

#[tokio::test]
async fn mission_action_resume_from_paused() {
    ensure_base_url();
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Paused, 2, 10)).await;
    let m = h.mission_action("m1", MissionAction::Resume).await.unwrap();
    assert_eq!(m.state, MissionState::Executing);
}

#[tokio::test]
async fn mission_action_resume_invalid() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Executing, 1, 10)).await;
    let err = h
        .mission_action("m1", MissionAction::Resume)
        .await
        .unwrap_err();
    assert!(err.contains("Cannot resume"));
}

#[tokio::test]
async fn mission_action_cancel_active() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("m1", "", MissionState::Executing, 1, 10)).await;
    let m = h.mission_action("m1", MissionAction::Cancel).await.unwrap();
    assert_eq!(m.state, MissionState::Cancelled);
}

#[tokio::test]
async fn mission_action_cancel_terminal_states() {
    let h = WebStateHandle::new_test();
    insert_mission(&h, mk_mission("mc", "", MissionState::Completed, 1, 10)).await;
    insert_mission(&h, mk_mission("mx", "", MissionState::Cancelled, 1, 10)).await;
    insert_mission(&h, mk_mission("mf", "", MissionState::Failed, 1, 10)).await;
    for id in ["mc", "mx", "mf"] {
        let err = h
            .mission_action(id, MissionAction::Cancel)
            .await
            .unwrap_err();
        assert!(err.contains("Cannot cancel"));
    }
}

#[tokio::test]
async fn mission_action_start_with_session_and_project_hits_network() {
    ensure_base_url();
    // project set so send_to_session proceeds past the empty-dir guard into the
    // reqwest call (connection refused → Err, swallowed by kick_mission_execution).
    let dir = std::env::temp_dir();
    let h = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    let mut m = mk_mission("m1", "sess-xyz", MissionState::Pending, 0, 10);
    m.project_index = 0;
    insert_mission(&h, m).await;
    let out = h.mission_action("m1", MissionAction::Start).await.unwrap();
    assert_eq!(out.state, MissionState::Executing);
}

// ── private prompt/send helpers (direct calls) ──────────────────────

#[tokio::test]
async fn kick_mission_execution_empty_session_warns() {
    let h = WebStateHandle::new_test();
    let m = mk_mission("m1", "", MissionState::Executing, 1, 10);
    // Should return without touching the network (empty session_id).
    h.kick_mission_execution(&m).await;
}

#[tokio::test]
async fn send_evaluator_prompt_empty_history_no_project() {
    let h = WebStateHandle::new_test();
    let m = mk_mission("m1", "sess", MissionState::Evaluating, 1, 10);
    // No projects → send_to_session returns Err on empty dir, swallowed.
    h.send_evaluator_prompt(&m).await;
}

#[tokio::test]
async fn send_evaluator_prompt_with_history_and_unlimited() {
    let h = WebStateHandle::new_test();
    let mut m = mk_mission("m1", "sess", MissionState::Evaluating, 2, 0); // max 0 → ∞
    m.eval_history.push(EvalRecord {
        iteration: 1,
        verdict: EvalVerdict::Continue,
        summary: "made progress".to_string(),
        next_step: Some("keep going".to_string()),
        timestamp: Utc::now().to_rfc3339(),
    });
    h.send_evaluator_prompt(&m).await;
}

#[tokio::test]
async fn send_continuation_prompt_with_and_without_next_step() {
    let h = WebStateHandle::new_test();
    let m = mk_mission("m1", "sess", MissionState::Executing, 2, 10);
    h.send_continuation_prompt(&m, Some("do the next thing"))
        .await;
    h.send_continuation_prompt(&m, None).await;
}

#[tokio::test]
async fn send_to_session_empty_dir_errors() {
    let h = WebStateHandle::new_test();
    let err = h
        .send_to_session("sess", &0, "hello", None)
        .await
        .unwrap_err();
    assert_eq!(err, "No project directory found");
}

#[tokio::test]
async fn send_to_session_with_project_connection_refused() {
    ensure_base_url();
    let dir = std::env::temp_dir();
    let h = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    let model = crate::web::types::ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude".to_string(),
    };
    // With and without model override; both hit reqwest → connection refused.
    let e1 = h
        .send_to_session("sess", &0, "hi", Some(&model))
        .await
        .unwrap_err();
    assert!(e1.starts_with("Failed to send message"));
    let e2 = h.send_to_session("sess", &0, "hi", None).await.unwrap_err();
    assert!(e2.starts_with("Failed to send message"));
}

#[tokio::test]
async fn parse_latest_eval_response_empty_dir() {
    let h = WebStateHandle::new_test();
    let m = mk_mission("m1", "sess", MissionState::Evaluating, 1, 10);
    let res = h.parse_latest_eval_response(&m).await;
    assert!(matches!(res.verdict, EvalVerdict::Continue));
    assert_eq!(res.summary, "Could not read session");
}

#[tokio::test]
async fn parse_latest_eval_response_with_project_fetch_error() {
    ensure_base_url();
    let dir = std::env::temp_dir();
    let h = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    let m = mk_mission("m1", "sess", MissionState::Evaluating, 1, 10);
    let res = h.parse_latest_eval_response(&m).await;
    assert!(matches!(res.verdict, EvalVerdict::Continue));
    assert!(res.summary.starts_with("Fetch error"));
}

// ── EvalResult::default_continue ────────────────────────────────────

#[test]
fn eval_result_default_continue() {
    let r = EvalResult::default_continue("because");
    assert!(matches!(r.verdict, EvalVerdict::Continue));
    assert_eq!(r.summary, "because");
    assert!(r.next_step.is_none());
}

// ── extract_message_text ────────────────────────────────────────────

#[test]
fn extract_text_from_parts() {
    let msg = serde_json::json!({
        "info": { "parts": [
            { "type": "text", "text": "line one" },
            { "type": "tool", "text": "ignored" },
            { "type": "text", "text": "line two" }
        ]}
    });
    assert_eq!(extract_message_text(&msg), "line one\nline two");
}

#[test]
fn extract_text_parts_without_text_falls_to_content() {
    let msg = serde_json::json!({
        "info": {
            "parts": [ { "type": "tool", "text": "x" } ],
            "content": [ { "type": "text", "text": "from content" } ]
        }
    });
    assert_eq!(extract_message_text(&msg), "from content");
}

#[test]
fn extract_text_from_content() {
    let msg = serde_json::json!({
        "info": { "content": [
            { "type": "text", "text": "c1" },
            { "type": "image", "text": "nope" },
            { "type": "text", "text": "c2" }
        ]}
    });
    assert_eq!(extract_message_text(&msg), "c1\nc2");
}

#[test]
fn extract_text_content_empty_returns_empty() {
    let msg = serde_json::json!({ "info": { "content": [] } });
    assert_eq!(extract_message_text(&msg), "");
}

#[test]
fn extract_text_no_parts_no_content() {
    let msg = serde_json::json!({ "info": { "role": "assistant" } });
    assert_eq!(extract_message_text(&msg), "");
}

// ── parse_eval_json ─────────────────────────────────────────────────

#[test]
fn parse_eval_json_valid_verdicts() {
    let a = parse_eval_json(r#"{"verdict":"achieved","summary":"done"}"#);
    assert!(matches!(a.verdict, EvalVerdict::Achieved));
    assert_eq!(a.summary, "done");

    let c = parse_eval_json(r#"{"verdict":"continue","summary":"more"}"#);
    assert!(matches!(c.verdict, EvalVerdict::Continue));

    let b = parse_eval_json(r#"{"verdict":"blocked","summary":"stuck"}"#);
    assert!(matches!(b.verdict, EvalVerdict::Blocked));

    let f = parse_eval_json(r#"{"verdict":"failed","summary":"nope"}"#);
    assert!(matches!(f.verdict, EvalVerdict::Failed));
}

#[test]
fn parse_eval_json_unknown_verdict_defaults_continue() {
    let r = parse_eval_json(r#"{"verdict":"weird","summary":"s"}"#);
    assert!(matches!(r.verdict, EvalVerdict::Continue));
}

#[test]
fn parse_eval_json_empty_summary_gets_placeholder() {
    let r = parse_eval_json(r#"{"verdict":"achieved"}"#);
    assert_eq!(r.summary, "Evaluation complete");
}

#[test]
fn parse_eval_json_with_surrounding_text_and_next_step() {
    let text = "Here is my answer:\n{\"verdict\":\"continue\",\"summary\":\"ok\",\"next_step\":\"go\"}\nThanks!";
    let r = parse_eval_json(text);
    assert!(matches!(r.verdict, EvalVerdict::Continue));
    assert_eq!(r.next_step.as_deref(), Some("go"));
}

#[test]
fn parse_eval_json_heuristic_branches() {
    let a = parse_eval_json("The goal has been met successfully.");
    assert!(matches!(a.verdict, EvalVerdict::Achieved));

    let b = parse_eval_json("I am blocked and need user input.");
    assert!(matches!(b.verdict, EvalVerdict::Blocked));

    let f = parse_eval_json("This is not achievable at all.");
    assert!(matches!(f.verdict, EvalVerdict::Failed));

    let c = parse_eval_json("Just some neutral prose without keywords.");
    assert!(matches!(c.verdict, EvalVerdict::Continue));
    assert!(c.summary.starts_with("Just some neutral"));
}

#[test]
fn parse_eval_json_open_brace_no_close_falls_to_heuristic() {
    // '{' present but no '}' → json_str = whole text → parse fails → heuristic.
    let r = parse_eval_json("{ verdict achieved but broken json");
    assert!(matches!(r.verdict, EvalVerdict::Achieved));
}

#[test]
fn parse_eval_json_long_text_truncated_to_200_chars() {
    let long = "z".repeat(500);
    let r = parse_eval_json(&long);
    assert!(matches!(r.verdict, EvalVerdict::Continue));
    assert_eq!(r.summary.chars().count(), 200);
}
