//! Generated tests for assistant.rs — mission-loop HTTP SUCCESS paths, driven
//! against a mock opencode upstream (`start_mock_upstream` + `scope_base_url`).
//!
//! These cover the awaited request futures that connection-refused tests can't:
//! `send_to_session` (2xx + non-2xx), `parse_latest_eval_response` (real parse),
//! `create_session_for_routine` (id / no-id / non-2xx), the private prompt
//! senders, and every verdict arm of `on_mission_evaluation_complete`
//! (Achieved→Completed, Failed→Failed, Blocked→Paused, Continue→Executing).
use super::*;
use crate::web::test_support::{scope_base_url, start_mock_upstream};
use crate::web::web_state::WebStateHandle;

use axum::routing::{get, post};

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

fn temp_project_handle() -> WebStateHandle {
    WebStateHandle::new_test_with_projects(vec![("p".to_string(), std::env::temp_dir())])
}

/// A mock upstream whose `GET /session/{id}/message` returns a single assistant
/// message whose text is the given evaluation JSON, and whose
/// `POST /session/{id}/message` and `POST /session` succeed.
fn eval_mock(verdict: &str) -> axum::Router {
    let text = format!(
        "{{\"verdict\":\"{verdict}\",\"summary\":\"eval summary\",\"next_step\":\"do the next thing\"}}"
    );
    let body = serde_json::json!([{
        "info": {
            "role": "assistant",
            "time": { "created": 10 },
            "parts": [{ "type": "text", "text": text }]
        }
    }]);
    let get_handler = move || {
        let body = body.clone();
        async move { axum::Json(body) }
    };
    axum::Router::new()
        .route(
            "/session/{id}/message",
            get(get_handler).post(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
        )
        .route(
            "/session",
            post(|| async { axum::Json(serde_json::json!({ "id": "new-sess" })) }),
        )
}

// ── send_to_session (awaited success + rejection) ───────────────────

#[tokio::test]
async fn send_to_session_success_returns_ok() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        post(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let res = scope_base_url(base, async move {
        h.send_to_session("sess-1", &0, "hello", None).await
    })
    .await;
    assert!(res.is_ok(), "expected Ok, got {res:?}");
}

#[tokio::test]
async fn send_to_session_success_with_model_override() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        post(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let model = crate::web::types::ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude".to_string(),
    };
    let res = scope_base_url(base, async move {
        h.send_to_session("sess-1", &0, "hi", Some(&model)).await
    })
    .await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn send_to_session_upstream_rejects_maps_error() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        post(|| async { (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let err = scope_base_url(base, async move {
        h.send_to_session("sess-1", &0, "hi", None).await
    })
    .await
    .unwrap_err();
    assert!(err.contains("Upstream rejected message"), "got {err}");
    assert!(err.contains("500"));
}

// ── parse_latest_eval_response (real parse over the wire) ────────────

#[tokio::test]
async fn parse_latest_eval_response_parses_achieved() {
    let base = start_mock_upstream(eval_mock("achieved")).await;
    let h = temp_project_handle();
    let m = mk_mission("m1", "sess", MissionState::Evaluating, 1, 10);
    let res = scope_base_url(base, async move { h.parse_latest_eval_response(&m).await }).await;
    assert!(matches!(res.verdict, EvalVerdict::Achieved));
    assert_eq!(res.summary, "eval summary");
    assert_eq!(res.next_step.as_deref(), Some("do the next thing"));
}

#[tokio::test]
async fn parse_latest_eval_response_non_json_body_is_parse_error() {
    // GET returns a JSON string (not an object/array of messages parsable as
    // eval) — actually returns plain text so resp.json() fails → Parse error.
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async { "this is not json at all" }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let m = mk_mission("m1", "sess", MissionState::Evaluating, 1, 10);
    let res = scope_base_url(base, async move { h.parse_latest_eval_response(&m).await }).await;
    assert!(matches!(res.verdict, EvalVerdict::Continue));
    assert!(res.summary.starts_with("Parse error"), "got {}", res.summary);
}

#[tokio::test]
async fn parse_latest_eval_response_no_assistant_message() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async { axum::Json(serde_json::json!([])) }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let m = mk_mission("m1", "sess", MissionState::Evaluating, 1, 10);
    let res = scope_base_url(base, async move { h.parse_latest_eval_response(&m).await }).await;
    assert!(matches!(res.verdict, EvalVerdict::Continue));
    assert_eq!(res.summary, "No assistant response found");
}

// ── create_session_for_routine (id / no-id / non-2xx) ───────────────

#[tokio::test]
async fn create_session_for_routine_success_returns_id() {
    let mock = axum::Router::new().route(
        "/session",
        post(|| async { axum::Json(serde_json::json!({ "id": "sess-created-42" })) }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let id = scope_base_url(base, async move { h.create_session_for_routine(0).await })
        .await
        .unwrap();
    assert_eq!(id, "sess-created-42");
}

#[tokio::test]
async fn create_session_for_routine_missing_id_errors() {
    let mock = axum::Router::new().route(
        "/session",
        post(|| async { axum::Json(serde_json::json!({ "no_id": true })) }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let err = scope_base_url(base, async move { h.create_session_for_routine(0).await })
        .await
        .unwrap_err();
    assert_eq!(err, "No session ID in response");
}

#[tokio::test]
async fn create_session_for_routine_non_success_status_errors() {
    let mock = axum::Router::new().route(
        "/session",
        post(|| async { (axum::http::StatusCode::BAD_GATEWAY, "nope") }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let err = scope_base_url(base, async move { h.create_session_for_routine(0).await })
        .await
        .unwrap_err();
    assert!(err.starts_with("Failed to create session: HTTP 502"), "got {err}");
}

// ── private prompt senders reach the mock (no-op on success) ─────────

#[tokio::test]
async fn kick_and_prompts_hit_success_upstream() {
    let base = start_mock_upstream(eval_mock("continue")).await;
    let h = temp_project_handle();
    let m = mk_mission("m1", "sess", MissionState::Executing, 1, 10);
    scope_base_url(base, async move {
        h.kick_mission_execution(&m).await;
        h.send_evaluator_prompt(&m).await;
        h.send_continuation_prompt(&m, Some("next")).await;
        h.send_continuation_prompt(&m, None).await;
    })
    .await;
}

// ── on_mission_evaluation_complete verdict arms ─────────────────────

#[tokio::test]
async fn on_eval_complete_achieved_marks_completed() {
    let base = start_mock_upstream(eval_mock("achieved")).await;
    let h = temp_project_handle();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 2, 10)).await;
    let h2 = h.clone();
    scope_base_url(base, async move { h2.on_mission_evaluation_complete("sess").await }).await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Completed);
    assert_eq!(m.iteration, 2); // unchanged
    assert!(matches!(m.last_verdict, Some(EvalVerdict::Achieved)));
    assert_eq!(m.last_eval_summary.as_deref(), Some("eval summary"));
    assert_eq!(m.eval_history.len(), 1);
    assert_eq!(m.eval_history[0].next_step.as_deref(), Some("do the next thing"));
}

#[tokio::test]
async fn on_eval_complete_failed_marks_failed() {
    let base = start_mock_upstream(eval_mock("failed")).await;
    let h = temp_project_handle();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 3, 10)).await;
    let h2 = h.clone();
    scope_base_url(base, async move { h2.on_mission_evaluation_complete("sess").await }).await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Failed);
    assert!(matches!(m.last_verdict, Some(EvalVerdict::Failed)));
}

#[tokio::test]
async fn on_eval_complete_blocked_marks_paused() {
    let base = start_mock_upstream(eval_mock("blocked")).await;
    let h = temp_project_handle();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 1, 10)).await;
    let h2 = h.clone();
    scope_base_url(base, async move { h2.on_mission_evaluation_complete("sess").await }).await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Paused);
    assert!(matches!(m.last_verdict, Some(EvalVerdict::Blocked)));
}

#[tokio::test]
async fn on_eval_complete_continue_advances_and_sends_next() {
    // Continue under max → Executing, iteration bumped, continuation prompt sent
    // (POST hits the mock on the awaited path).
    let base = start_mock_upstream(eval_mock("continue")).await;
    let h = temp_project_handle();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 2, 10)).await;
    let h2 = h.clone();
    scope_base_url(base, async move { h2.on_mission_evaluation_complete("sess").await }).await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Executing);
    assert_eq!(m.iteration, 3);
    assert!(matches!(m.last_verdict, Some(EvalVerdict::Continue)));
}

#[tokio::test]
async fn on_eval_complete_continue_at_max_fails() {
    let base = start_mock_upstream(eval_mock("continue")).await;
    let h = temp_project_handle();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 4, 4)).await;
    let h2 = h.clone();
    scope_base_url(base, async move { h2.on_mission_evaluation_complete("sess").await }).await;
    let m = h.get_mission("m1").await.unwrap();
    assert_eq!(m.state, MissionState::Failed);
    assert_eq!(m.iteration, 4);
}

#[tokio::test]
async fn try_advance_evaluating_end_to_end_over_upstream() {
    // Full route: try_advance → on_mission_evaluation_complete → Achieved.
    let base = start_mock_upstream(eval_mock("achieved")).await;
    let h = temp_project_handle();
    insert_mission(&h, mk_mission("m1", "sess", MissionState::Evaluating, 1, 10)).await;
    let h2 = h.clone();
    scope_base_url(base, async move { h2.try_advance_mission("sess").await }).await;
    assert_eq!(h.get_mission("m1").await.unwrap().state, MissionState::Completed);
}
