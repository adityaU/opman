//! Generated tests for assistant.rs — `execute_routine` SUCCESS paths driven
//! against a mock opencode upstream. Covers the NewSession branch (create
//! session over the wire, then send) and the ExistingSession branch (send with
//! and without a model override), asserting a "completed" run is recorded.
use super::*;
use crate::web::test_support::{scope_base_url, start_mock_upstream};
use crate::web::web_state::WebStateHandle;

use axum::routing::post;

fn temp_project_handle() -> WebStateHandle {
    WebStateHandle::new_test_with_projects(vec![("p".to_string(), std::env::temp_dir())])
}

fn mk_routine(name: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        name: name.to_string(),
        trigger: RoutineTrigger::Manual,
        enabled: true,
        cron_expr: None,
        timezone: None,
        target_mode: None,
        session_id: None,
        project_index: Some(0),
        prompt: Some("do the work".to_string()),
        provider_id: None,
        model_id: None,
    }
}

/// Mock upstream that satisfies both `POST /session` (returns an id) and
/// `POST /session/{id}/message` (2xx).
fn ok_mock() -> axum::Router {
    axum::Router::new()
        .route(
            "/session",
            post(|| async { axum::Json(serde_json::json!({ "id": "sess-new-0123456789ab" })) }),
        )
        .route(
            "/session/{id}/message",
            post(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
        )
}

#[tokio::test]
async fn execute_routine_new_session_success_records_completed() {
    let base = start_mock_upstream(ok_mock()).await;
    let h = temp_project_handle();
    let mut req = mk_routine("ns");
    req.target_mode = Some(RoutineTargetMode::NewSession);
    let r = h.create_routine(req).await;

    let h2 = h.clone();
    let rid = r.id.clone();
    let run = scope_base_url(base, async move { h2.execute_routine(&rid).await })
        .await
        .unwrap();

    assert_eq!(run.status, "completed");
    assert!(
        run.summary.starts_with("Sent message to session"),
        "got {}",
        run.summary
    );
    assert_eq!(
        run.target_session_id.as_deref(),
        Some("sess-new-0123456789ab")
    );
    assert!(run.duration_ms.is_some());
    // Persisted + last_error cleared on the routine.
    let (_, runs) = h.list_routines().await;
    assert!(runs.iter().any(|x| x.status == "completed"));
}

#[tokio::test]
async fn execute_routine_existing_session_success() {
    let base = start_mock_upstream(ok_mock()).await;
    let h = temp_project_handle();
    let mut req = mk_routine("ex");
    req.target_mode = Some(RoutineTargetMode::ExistingSession);
    req.session_id = Some("sess-existing-abcdef".to_string());
    let r = h.create_routine(req).await;

    let h2 = h.clone();
    let rid = r.id.clone();
    let run = scope_base_url(base, async move { h2.execute_routine(&rid).await })
        .await
        .unwrap();

    assert_eq!(run.status, "completed");
    assert_eq!(
        run.target_session_id.as_deref(),
        Some("sess-existing-abcdef")
    );
}

#[tokio::test]
async fn execute_routine_existing_session_with_model_override_success() {
    let base = start_mock_upstream(ok_mock()).await;
    let h = temp_project_handle();
    let mut req = mk_routine("exm");
    req.target_mode = Some(RoutineTargetMode::ExistingSession);
    req.session_id = Some("sess-model-xyz".to_string());
    req.provider_id = Some("anthropic".to_string());
    req.model_id = Some("claude".to_string());
    let r = h.create_routine(req).await;

    let h2 = h.clone();
    let rid = r.id.clone();
    let run = scope_base_url(base, async move { h2.execute_routine(&rid).await })
        .await
        .unwrap();
    assert_eq!(run.status, "completed");
}

#[tokio::test]
async fn execute_routine_new_session_creation_fails_records_failed() {
    // POST /session returns a non-2xx → create_session_for_routine errors →
    // failed run recorded and the error propagated.
    let mock = axum::Router::new().route(
        "/session",
        post(|| async { (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let mut req = mk_routine("nsfail");
    req.target_mode = Some(RoutineTargetMode::NewSession);
    let r = h.create_routine(req).await;

    let h2 = h.clone();
    let rid = r.id.clone();
    let err = scope_base_url(base, async move { h2.execute_routine(&rid).await })
        .await
        .unwrap_err();
    assert!(err.starts_with("Failed to create session"), "got {err}");
    let (_, runs) = h.list_routines().await;
    assert!(runs.iter().any(|x| x.status == "failed"));
}

#[tokio::test]
async fn execute_routine_send_fails_records_failed_run() {
    // Session exists but the message POST is rejected → failed run with the
    // sending session id captured.
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        post(|| async { (axum::http::StatusCode::SERVICE_UNAVAILABLE, "down") }),
    );
    let base = start_mock_upstream(mock).await;
    let h = temp_project_handle();
    let mut req = mk_routine("sendfail");
    req.target_mode = Some(RoutineTargetMode::ExistingSession);
    req.session_id = Some("sess-send-fail".to_string());
    let r = h.create_routine(req).await;

    let h2 = h.clone();
    let rid = r.id.clone();
    let err = scope_base_url(base, async move { h2.execute_routine(&rid).await })
        .await
        .unwrap_err();
    assert!(err.contains("Upstream rejected message"), "got {err}");
    let (_, runs) = h.list_routines().await;
    let failed = runs.iter().find(|x| x.status == "failed").unwrap();
    assert_eq!(failed.target_session_id.as_deref(), Some("sess-send-fail"));
}
