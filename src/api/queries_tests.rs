use super::*;
use axum::{http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};

async fn spawn(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{}", addr)
}

/// Bind an ephemeral port then drop it so connecting is refused immediately.
async fn dead_url() -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    format!("http://{}", addr)
}

fn json_get(path: &'static str, body: Value) -> Router {
    Router::new().route(
        path,
        get(move || {
            let b = body.clone();
            async move { Json(b) }
        }),
    )
}

// ---- fetch_project_info -------------------------------------------------

#[tokio::test]
async fn fetch_project_info_success() {
    let app = json_get("/info", json!({ "directory": "/srv/proj", "version": "1.2.3" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let info = client.fetch_project_info(&base, "/tmp").await.unwrap();
    assert_eq!(info.directory, "/srv/proj");
    assert_eq!(info.version, "1.2.3");
}

#[tokio::test]
async fn fetch_project_info_defaults_missing_fields() {
    let app = json_get("/info", json!({}));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let info = client.fetch_project_info(&base, "/tmp").await.unwrap();
    assert_eq!(info.directory, "");
    assert_eq!(info.version, "");
}

#[tokio::test]
async fn fetch_project_info_malformed_errors() {
    let app = Router::new().route("/info", get(|| async { "not json" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.fetch_project_info(&base, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse project info response"));
}

#[tokio::test]
async fn fetch_project_info_connection_error() {
    let client = ApiClient::new();
    let err = client.fetch_project_info(&dead_url().await, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch project info"));
}

// ---- health_check -------------------------------------------------------

#[tokio::test]
async fn health_check_ok_true() {
    let app = Router::new().route("/health", get(|| async { Json(json!({"ok": true})) }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client.health_check(&base).await.unwrap());
}

#[tokio::test]
async fn health_check_non_success_false() {
    let app = Router::new().route(
        "/health",
        get(|| async { (StatusCode::SERVICE_UNAVAILABLE, "down") }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(!client.health_check(&base).await.unwrap());
}

#[tokio::test]
async fn health_check_connection_error_false() {
    let client = ApiClient::new();
    // Err arm -> Ok(false).
    assert!(!client.health_check(&dead_url().await).await.unwrap());
}

// ---- fetch_context_window -----------------------------------------------

#[tokio::test]
async fn fetch_context_window_found() {
    let body = json!([
        { "id": "p1", "models": { "other": { "limit": { "context": 111 } } } },
        { "id": "p2", "models": { "target": { "limit": { "context": 128000 } } } }
    ]);
    let app = json_get("/provider", body);
    let base = spawn(app).await;
    let client = ApiClient::new();
    let ctx = client
        .fetch_context_window(&base, "/tmp", "target")
        .await
        .unwrap();
    assert_eq!(ctx, 128000);
}

#[tokio::test]
async fn fetch_context_window_model_not_found_falls_back() {
    let body = json!([{ "id": "p1", "models": { "x": { "limit": { "context": 5 } } } }]);
    let app = json_get("/provider", body);
    let base = spawn(app).await;
    let client = ApiClient::new();
    let ctx = client
        .fetch_context_window(&base, "/tmp", "missing")
        .await
        .unwrap();
    assert_eq!(ctx, 200_000);
}

#[tokio::test]
async fn fetch_context_window_model_without_limit_falls_back() {
    // model present but no limit.context -> inner if fails -> fallback.
    let body = json!([{ "models": { "target": { "name": "no limit" } } }]);
    let app = json_get("/provider", body);
    let base = spawn(app).await;
    let client = ApiClient::new();
    let ctx = client
        .fetch_context_window(&base, "/tmp", "target")
        .await
        .unwrap();
    assert_eq!(ctx, 200_000);
}

#[tokio::test]
async fn fetch_context_window_non_array_falls_back() {
    let app = json_get("/provider", json!({ "not": "an array" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let ctx = client
        .fetch_context_window(&base, "/tmp", "m")
        .await
        .unwrap();
    assert_eq!(ctx, 200_000);
}

#[tokio::test]
async fn fetch_context_window_malformed_errors() {
    let app = Router::new().route("/provider", get(|| async { "boom" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .fetch_context_window(&base, "/tmp", "m")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse provider response"));
}

#[tokio::test]
async fn fetch_context_window_connection_error() {
    let client = ApiClient::new();
    let err = client
        .fetch_context_window(&dead_url().await, "/tmp", "m")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch provider info"));
}

// ---- fetch_todos --------------------------------------------------------

#[tokio::test]
async fn fetch_todos_success() {
    let body = json!([
        { "content": "do a", "status": "pending", "priority": "high" },
        { "content": "do b", "status": "completed", "priority": "low" }
    ]);
    let app = json_get("/session/{id}/todo", body);
    let base = spawn(app).await;
    let client = ApiClient::new();
    let todos = client.fetch_todos(&base, "/tmp", "s1").await.unwrap();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].content, "do a");
    assert_eq!(todos[1].status, "completed");
}

#[tokio::test]
async fn fetch_todos_malformed_errors() {
    let app = Router::new().route("/session/{id}/todo", get(|| async { "nope" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.fetch_todos(&base, "/tmp", "s1").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse todos response"));
}

#[tokio::test]
async fn fetch_todos_connection_error() {
    let client = ApiClient::new();
    let err = client.fetch_todos(&dead_url().await, "/tmp", "s1").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch todos"));
}

// ---- fetch_providers ----------------------------------------------------

#[tokio::test]
async fn fetch_providers_success_returns_raw() {
    let body = json!([{ "id": "anthropic", "models": {} }]);
    let app = json_get("/provider", body.clone());
    let base = spawn(app).await;
    let client = ApiClient::new();
    let out = client.fetch_providers(&base, "/tmp").await.unwrap();
    assert_eq!(out, body);
}

#[tokio::test]
async fn fetch_providers_malformed_errors() {
    let app = Router::new().route("/provider", get(|| async { "xx" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.fetch_providers(&base, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse provider response"));
}

#[tokio::test]
async fn fetch_providers_connection_error() {
    let client = ApiClient::new();
    let err = client.fetch_providers(&dead_url().await, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch providers"));
}

// ---- list_commands ------------------------------------------------------

#[tokio::test]
async fn list_commands_success_returns_raw() {
    let body = json!([{ "name": "compact" }, { "name": "init" }]);
    let app = json_get("/command", body.clone());
    let base = spawn(app).await;
    let client = ApiClient::new();
    let out = client.list_commands(&base, "/tmp").await.unwrap();
    assert_eq!(out, body);
}

#[tokio::test]
async fn list_commands_malformed_errors() {
    let app = Router::new().route("/command", get(|| async { "zz" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.list_commands(&base, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse command list response"));
}

#[tokio::test]
async fn list_commands_connection_error() {
    let client = ApiClient::new();
    let err = client.list_commands(&dead_url().await, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to list commands"));
}
