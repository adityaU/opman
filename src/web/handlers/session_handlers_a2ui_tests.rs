//! A2UI callbacks reach the runner that owns the session.

use std::sync::Arc;

use axum::extract::Path;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::runner::{HttpRunner, Runner, RunnerKind};
use crate::web::test_support::{send_json, test_router, test_server_state};

/// A claude-slot session's button press must go to claude, not to the default runner.
#[tokio::test]
async fn a2ui_callback_goes_to_the_sessions_own_runner() {
    let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let record = seen.clone();
    let app = Router::new().route(
        "/session/{id}/prompt_async",
        post(move |Path(id): Path<String>, Json(body): Json<Value>| {
            let record = record.clone();
            async move {
                record.lock().await.push((id, body));
                Json(json!({ "ok": true }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().unwrap().to_string();
    let state = test_server_state();
    state
        .web_state
        .add_project(&dir, Some("proj"))
        .await
        .expect("add project");
    let claude = HttpRunner::new(RunnerKind::Claude, url, reqwest::Client::new());
    state
        .runner_registry
        .install(RunnerKind::Claude, Arc::new(claude) as Arc<dyn Runner>);
    state
        .runner_registry
        .ensure_binding("s1", RunnerKind::Claude, &dir)
        .await;

    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/s1/a2ui/callback",
        Some(json!({ "callback_id": "cb1" })),
    )
    .await;

    assert!(status.is_success(), "got {status}");
    let seen = seen.lock().await;
    assert_eq!(seen.len(), 1, "claude should receive exactly one callback");
    assert_eq!(seen[0].0, "s1");
    assert!(seen[0].1.to_string().contains("cb1"));
}
