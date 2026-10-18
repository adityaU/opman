use super::*;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::web::test_support::{
    scope_base_url, start_mock_upstream, test_router, test_server_state_with_projects,
};

fn state() -> ServerState {
    test_server_state_with_projects(vec![
        ("repo".into(), "/repo".into()),
        ("other".into(), "/other".into()),
    ])
}

async fn call(
    state: &ServerState,
    token: &str,
    operation: &str,
    args: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/internal/scheduler")
        .header("content-type", "application/json")
        .header("x-internal-token", token)
        .body(Body::from(
            json!({
                "operation": operation, "project_dir": "/repo", "args": args,
            })
            .to_string(),
        ))
        .expect("valid request");
    let response = test_router(state.clone())
        .oneshot(request)
        .await
        .expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    (status, serde_json::from_slice(&body).expect("JSON body"))
}

fn creation() -> Value {
    json!({ "name": "Daily check", "prompt": "Review changes", "cron_expr": "0 9 * * *" })
}

const TOKEN: &str = "test-internal-token";

#[tokio::test]
async fn missing_wrong_and_empty_server_tokens_are_rejected() {
    let mut state = state();
    for token in ["", "wrong"] {
        assert_eq!(
            call(&state, token, "create", creation()).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    state.internal_token.clear();
    assert_eq!(
        call(&state, "", "create", creation()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(state.web_state.list_routines().await.0.is_empty());
}

#[tokio::test]
async fn invalid_schedules_and_fields_do_not_create_routines() {
    let state = state();
    for changes in [
        json!({"cron_expr": "bad"}),
        json!({"cron_expr": "99 9 * * *"}),
        json!({"timezone": "Wrong/Zone"}),
        json!({"name": " "}),
        json!({"prompt": " "}),
        json!({"project_index": 88}),
        json!({"project_index": 1}),
        json!({"target_mode": "existing_session"}),
        json!({"target_mode": "existing_session", "session_id": "missing"}),
    ] {
        let mut args = creation();
        args.as_object_mut()
            .expect("object")
            .extend(changes.as_object().expect("object").clone());
        assert_eq!(
            call(&state, TOKEN, "create", args).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert!(state.web_state.list_routines().await.0.is_empty());
}

#[tokio::test]
async fn create_list_pause_resume_and_delete_use_routines() {
    let state = state();
    let (status, created) = call(&state, TOKEN, "create", creation()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["trigger"], "scheduled");
    assert_eq!(created["target_mode"], "new_session");
    assert_eq!(created["project_index"], 0);
    let id = created["id"].as_str().expect("routine ID");
    let (status, listed) = call(&state, TOKEN, "list", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["routines"][0]["id"], id);
    assert_eq!(listed["runs"], json!([]));
    for enabled in [false, true] {
        let (status, updated) = call(
            &state,
            TOKEN,
            "update",
            json!({"routine_id": id, "enabled": enabled}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["enabled"], enabled);
    }
    let (status, deleted) = call(&state, TOKEN, "delete", json!({"routine_id": id})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);
    assert!(state.web_state.list_routines().await.0.is_empty());
}

#[tokio::test]
async fn invalid_updates_leave_the_existing_schedule_unchanged() {
    let state = state();
    let (_, created) = call(&state, TOKEN, "create", creation()).await;
    let id = created["id"].as_str().expect("routine ID");
    let (status, _) = call(
        &state,
        TOKEN,
        "update",
        json!({"routine_id":id, "cron_expr":"invalid", "enabled":false}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, listed) = call(&state, TOKEN, "list", json!({})).await;
    assert_eq!(listed["routines"][0]["cron_expr"], "0 9 * * *");
    assert_eq!(listed["routines"][0]["enabled"], true);
}

#[tokio::test]
async fn list_filters_routines_and_history_by_project() {
    let state = state();
    let args: CreateArgs = serde_json::from_value(creation()).expect("create args");
    let request = creation_request(args.fields, 1).expect("request");
    let created = state.web_state.create_routine(request).await;
    let id = created.id.as_str();
    state
        .web_state
        .record_routine_run(id, "test".into(), None, None, "failed")
        .await;
    let (_, listed) = call(&state, TOKEN, "list", json!({})).await;
    assert_eq!(listed, json!({"routines": [], "runs": []}));
    let (_, listed) = call(&state, TOKEN, "list", json!({"all_projects":true})).await;
    assert_eq!(listed["routines"][0]["id"], id);
    assert_eq!(listed["runs"][0]["routine_id"], id);
    assert_eq!(
        call(&state, TOKEN, "delete", json!({"routine_id":id}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn run_surfaces_target_errors_and_records_failed_run() {
    let state = state();
    let upstream = start_mock_upstream(axum::Router::new().route(
        "/session",
        axum::routing::post(|| async {
            (StatusCode::SERVICE_UNAVAILABLE, "test runner unavailable")
        }),
    ))
    .await;
    let (_, created) = call(&state, TOKEN, "create", creation()).await;
    let id = created["id"].as_str().expect("routine ID");
    let (status, result) = scope_base_url(
        upstream,
        call(&state, TOKEN, "run", json!({"routine_id":id})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(result["error"]
        .as_str()
        .expect("error")
        .contains("Failed to create session"));
    let (_, listed) = call(&state, TOKEN, "list", json!({})).await;
    assert_eq!(listed["runs"][0]["status"], "failed");
    assert_eq!(
        call(&state, TOKEN, "run", json!({"routine_id":"missing"}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn unknown_projects_are_rejected_before_mutation() {
    let state = state();
    assert!(validation::resolve_project(&state, "/unknown")
        .await
        .is_err());
    assert!(validation::resolve_project(&state, "repo").await.is_err());
    assert!(state.web_state.list_routines().await.0.is_empty());
}
