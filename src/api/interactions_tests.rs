use super::*;
use axum::{extract::Path, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};

// ---- mock server helpers ------------------------------------------------

/// Spawn `router` on an ephemeral 127.0.0.1 port and return its base URL.
async fn spawn(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{}", addr)
}

/// A base URL that nothing is listening on (forces connection-refused).
/// Bind an ephemeral port then drop it so connecting is refused immediately.
async fn dead_url() -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    format!("http://{}", addr)
}

// ---- CommandError -------------------------------------------------------

#[test]
fn command_error_display_and_trait() {
    let e = CommandError {
        status: 503,
        message: "boom".into(),
    };
    assert_eq!(format!("{}", e), "HTTP 503 — boom");
    // Debug is derived.
    let dbg = format!("{:?}", e);
    assert!(dbg.contains("503"));
    // std::error::Error is implemented (source defaults to None).
    let as_err: &dyn std::error::Error = &e;
    assert!(as_err.source().is_none());
}

// ---- reply_permission ---------------------------------------------------

#[tokio::test]
async fn reply_permission_success() {
    let app = Router::new().route(
        "/permission/{id}/reply",
        post(|Path(_id): Path<String>, Json(_b): Json<Value>| async { Json(json!({"ok": true})) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let r = client
        .reply_permission(&base, "/tmp/proj", "req-1", "once")
        .await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn reply_permission_failure_status() {
    let app = Router::new().route(
        "/permission/{id}/reply",
        post(|| async { (StatusCode::FORBIDDEN, "nope".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .reply_permission(&base, "/tmp/proj", "req-1", "reject")
        .await
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("403"));
    assert!(msg.contains("nope"));
}

#[tokio::test]
async fn reply_permission_connection_error() {
    let client = ApiClient::new();
    let err = client
        .reply_permission(&dead_url().await, "/tmp", "r", "once")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to reply to permission request"));
}

// ---- reply_question -----------------------------------------------------

#[tokio::test]
async fn reply_question_success() {
    let app = Router::new().route(
        "/question/{id}/reply",
        post(|Json(_b): Json<Value>| async { Json(json!({})) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::with_client(reqwest::Client::new());
    let answers = vec![vec!["A".to_string()], vec!["custom".to_string()]];
    assert!(client
        .reply_question(&base, "/tmp/proj", "q-1", &answers)
        .await
        .is_ok());
}

#[tokio::test]
async fn reply_question_failure_status() {
    let app = Router::new().route(
        "/question/{id}/reply",
        post(|| async { (StatusCode::BAD_REQUEST, "bad".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .reply_question(&base, "/tmp", "q", &[])
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("400"));
}

#[tokio::test]
async fn reply_question_connection_error() {
    let client = ApiClient::new();
    let err = client
        .reply_question(&dead_url().await, "/tmp", "q", &[])
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to reply to question"));
}

// ---- reject_question ----------------------------------------------------

#[tokio::test]
async fn reject_question_success() {
    let app = Router::new().route("/question/{id}/reject", post(|| async { Json(json!({})) }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client.reject_question(&base, "/tmp", "q-9").await.is_ok());
}

#[tokio::test]
async fn reject_question_failure_status() {
    let app = Router::new().route(
        "/question/{id}/reject",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "err".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .reject_question(&base, "/tmp", "q")
        .await
        .unwrap_err();
    let m = format!("{}", err);
    assert!(m.contains("500") && m.contains("err"));
}

#[tokio::test]
async fn reject_question_connection_error() {
    let client = ApiClient::new();
    let err = client
        .reject_question(&dead_url().await, "/tmp", "q")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to reject question"));
}

// ---- execute_session_command --------------------------------------------

#[tokio::test]
async fn execute_command_success_echoes_body_without_model() {
    // Echo the request body back so we can assert the payload shape.
    let app = Router::new().route(
        "/session/{id}/command",
        post(|Json(b): Json<Value>| async move { Json(b) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let out = client
        .execute_session_command(&base, "/tmp", "s1", "/compact", "args here", None)
        .await
        .unwrap();
    assert_eq!(out["command"], "/compact");
    assert_eq!(out["arguments"], "args here");
    assert!(out.get("model").is_none());
}

#[tokio::test]
async fn execute_command_success_with_model() {
    let app = Router::new().route(
        "/session/{id}/command",
        post(|Json(b): Json<Value>| async move { Json(b) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let out = client
        .execute_session_command(&base, "/tmp", "s1", "/init", "", Some("anthropic/claude"))
        .await
        .unwrap();
    assert_eq!(out["model"], "anthropic/claude");
}

#[tokio::test]
async fn execute_command_failure_with_error_field() {
    let app = Router::new().route(
        "/session/{id}/command",
        post(|| async {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "nope-cmd" })),
            )
        }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .execute_session_command(&base, "/tmp", "s1", "/bad", "", None)
        .await
        .unwrap_err();
    // Downcasts to CommandError preserving status.
    let ce = err.downcast_ref::<CommandError>().expect("CommandError");
    assert_eq!(ce.status, 400);
    assert!(ce.message.contains("nope-cmd"));
    assert!(ce.message.contains("/bad"));
}

#[tokio::test]
async fn execute_command_failure_with_data_message_pointer() {
    let app = Router::new().route(
        "/session/{id}/command",
        post(|| async {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "data": { "message": "deep-err" } })),
            )
        }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .execute_session_command(&base, "/tmp", "s1", "/x", "", None)
        .await
        .unwrap_err();
    let ce = err.downcast_ref::<CommandError>().unwrap();
    assert_eq!(ce.status, 500);
    assert!(ce.message.contains("deep-err"));
}

#[tokio::test]
async fn execute_command_failure_unknown_error() {
    let app = Router::new().route(
        "/session/{id}/command",
        post(|| async { (StatusCode::BAD_GATEWAY, Json(json!({ "irrelevant": 1 }))) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .execute_session_command(&base, "/tmp", "s1", "/x", "", None)
        .await
        .unwrap_err();
    let ce = err.downcast_ref::<CommandError>().unwrap();
    assert_eq!(ce.status, 502);
    assert!(ce.message.contains("unknown error"));
}

#[tokio::test]
async fn execute_command_failure_non_json_body_defaults_null() {
    // Non-JSON body -> resp.json() fails -> Null -> "unknown error".
    let app = Router::new().route(
        "/session/{id}/command",
        post(|| async { (StatusCode::NOT_FOUND, "plain text".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .execute_session_command(&base, "/tmp", "s1", "/x", "", None)
        .await
        .unwrap_err();
    let ce = err.downcast_ref::<CommandError>().unwrap();
    assert_eq!(ce.status, 404);
    assert!(ce.message.contains("unknown error"));
}

#[tokio::test]
async fn execute_command_connection_error() {
    let client = ApiClient::new();
    let err = client
        .execute_session_command(&dead_url().await, "/tmp", "s1", "/x", "", None)
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to execute session command"));
}
