use super::*;
use axum::{http::StatusCode, routing::get, routing::post, Json, Router};
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

// ---- fetch_sessions -----------------------------------------------------

#[tokio::test]
async fn fetch_sessions_success() {
    let body = json!([
        { "id": "s1", "title": "First", "parentID": "", "directory": "/a",
          "time": { "created": 1, "updated": 2 } },
        { "id": "s2" }
    ]);
    let base = spawn(json_get("/session", body)).await;
    let client = ApiClient::new();
    let out = client.fetch_sessions(&base, "/tmp").await.unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "s1");
    assert_eq!(out[0].title, "First");
    assert_eq!(out[0].time.created, 1);
    // Defaulted fields on the minimal session.
    assert_eq!(out[1].id, "s2");
    assert_eq!(out[1].title, "");
}

#[tokio::test]
async fn fetch_sessions_malformed_errors() {
    let base = spawn(Router::new().route("/session", get(|| async { "nope" }))).await;
    let client = ApiClient::new();
    let err = client.fetch_sessions(&base, "/tmp").await.unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse session list response"));
}

#[tokio::test]
async fn fetch_sessions_connection_error() {
    let client = ApiClient::new();
    let err = client
        .fetch_sessions(&dead_url().await, "/tmp")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch sessions"));
}

// ---- fetch_session_status -----------------------------------------------

#[tokio::test]
async fn fetch_session_status_maps_types() {
    let body = json!({
        "s1": { "type": "busy" },
        "s2": { "type": "retry" },
        "s3": { "no_type": true }
    });
    let base = spawn(json_get("/session/status", body)).await;
    let client = ApiClient::new();
    let map = client.fetch_session_status(&base, "/tmp").await.unwrap();
    assert_eq!(map.get("s1").map(|s| s.as_str()), Some("busy"));
    assert_eq!(map.get("s2").map(|s| s.as_str()), Some("retry"));
    // Entry without a "type" string is skipped.
    assert!(!map.contains_key("s3"));
}

#[tokio::test]
async fn fetch_session_status_non_object_yields_empty() {
    let base = spawn(json_get("/session/status", json!([1, 2, 3]))).await;
    let client = ApiClient::new();
    let map = client.fetch_session_status(&base, "/tmp").await.unwrap();
    assert!(map.is_empty());
}

#[tokio::test]
async fn fetch_session_status_malformed_errors() {
    let base = spawn(Router::new().route("/session/status", get(|| async { "x" }))).await;
    let client = ApiClient::new();
    let err = client
        .fetch_session_status(&base, "/tmp")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse session status response"));
}

#[tokio::test]
async fn fetch_session_status_connection_error() {
    let client = ApiClient::new();
    let err = client
        .fetch_session_status(&dead_url().await, "/tmp")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch session status"));
}

// ---- select_session (fire-and-forget) -----------------------------------

#[tokio::test]
async fn select_session_success() {
    let app = Router::new().route(
        "/tui/select-session",
        post(|Json(_b): Json<Value>| async { Json(json!({})) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client.select_session(&base, "/tmp", "s1").await.is_ok());
}

#[tokio::test]
async fn select_session_connection_error() {
    let client = ApiClient::new();
    let err = client
        .select_session(&dead_url().await, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to select session"));
}

// ---- abort_session ------------------------------------------------------

#[tokio::test]
async fn abort_session_success() {
    let app = Router::new().route("/session/{id}/abort", post(|| async { Json(json!({})) }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client.abort_session(&base, "/tmp", "s1").await.is_ok());
}

#[tokio::test]
async fn abort_session_failure_status() {
    let app = Router::new().route(
        "/session/{id}/abort",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "cant".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.abort_session(&base, "/tmp", "s1").await.unwrap_err();
    let m = format!("{}", err);
    assert!(m.contains("500") && m.contains("cant"));
}

#[tokio::test]
async fn abort_session_connection_error() {
    let client = ApiClient::new();
    let err = client
        .abort_session(&dead_url().await, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to send abort request"));
}

// ---- unrevert_session ---------------------------------------------------

#[tokio::test]
async fn unrevert_session_success() {
    let app = Router::new().route("/session/{id}/unrevert", post(|| async { Json(json!({})) }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client.unrevert_session(&base, "/tmp", "s1").await.is_ok());
}

#[tokio::test]
async fn unrevert_session_failure_status() {
    let app = Router::new().route(
        "/session/{id}/unrevert",
        post(|| async { (StatusCode::BAD_REQUEST, "no".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .unrevert_session(&base, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Session unrevert rejected"));
}

#[tokio::test]
async fn unrevert_session_connection_error() {
    let client = ApiClient::new();
    let err = client
        .unrevert_session(&dead_url().await, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to send unrevert request"));
}

// ---- share_session ------------------------------------------------------

#[tokio::test]
async fn share_session_success() {
    let app = Router::new().route(
        "/session/{id}/share",
        post(|| async { Json(json!({ "url": "https://share/x" })) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let out = client.share_session(&base, "/tmp", "s1").await.unwrap();
    assert_eq!(out["url"], "https://share/x");
}

#[tokio::test]
async fn share_session_failure_with_error_field() {
    let app = Router::new().route(
        "/session/{id}/share",
        post(|| async { (StatusCode::FORBIDDEN, Json(json!({ "error": "denied" }))) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.share_session(&base, "/tmp", "s1").await.unwrap_err();
    let m = format!("{}", err);
    assert!(m.contains("403") && m.contains("denied"));
}

#[tokio::test]
async fn share_session_failure_unknown_error() {
    let app = Router::new().route(
        "/session/{id}/share",
        post(|| async { (StatusCode::BAD_GATEWAY, Json(json!({ "x": 1 }))) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.share_session(&base, "/tmp", "s1").await.unwrap_err();
    assert!(format!("{}", err).contains("unknown error"));
}

#[tokio::test]
async fn share_session_failure_non_json_body() {
    // Non-JSON error body -> Null -> "unknown error".
    let app = Router::new().route(
        "/session/{id}/share",
        post(|| async { (StatusCode::NOT_FOUND, "plain".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client.share_session(&base, "/tmp", "s1").await.unwrap_err();
    let m = format!("{}", err);
    assert!(m.contains("404") && m.contains("unknown error"));
}

#[tokio::test]
async fn share_session_connection_error() {
    let client = ApiClient::new();
    let err = client
        .share_session(&dead_url().await, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to send share request"));
}

// ---- revert_session (multi-endpoint) ------------------------------------

/// Build a router serving all three endpoints revert_session touches.
fn revert_router(session_body: Value, messages_body: Value, revert_status: StatusCode) -> Router {
    Router::new()
        .route(
            "/session/{id}",
            get(move || {
                let b = session_body.clone();
                async move { Json(b) }
            }),
        )
        .route(
            "/session/{id}/message",
            get(move || {
                let b = messages_body.clone();
                async move { Json(b) }
            }),
        )
        .route(
            "/session/{id}/revert",
            post(move |Json(_b): Json<Value>| async move {
                if revert_status.is_success() {
                    (revert_status, "".to_string())
                } else {
                    (revert_status, "revert-failed".to_string())
                }
            }),
        )
}

#[tokio::test]
async fn revert_session_uses_last_user_message_from_array() {
    let session_body = json!({ "id": "s1" });
    let messages_body = json!([
        { "info": { "role": "user", "id": "u1" } },
        { "info": { "role": "assistant", "id": "a1" } },
        { "info": { "role": "user", "id": "u2" } }
    ]);
    let base = spawn(revert_router(session_body, messages_body, StatusCode::OK)).await;
    let client = ApiClient::new();
    assert!(client.revert_session(&base, "/tmp", "s1").await.is_ok());
}

#[tokio::test]
async fn revert_session_falls_back_to_session_revert_pointer() {
    // No user message in the array -> fall back to session.revert.messageID.
    let session_body = json!({ "id": "s1", "revert": { "messageID": "mrev" } });
    let messages_body = json!([{ "info": { "role": "assistant", "id": "a1" } }]);
    let base = spawn(revert_router(session_body, messages_body, StatusCode::OK)).await;
    let client = ApiClient::new();
    assert!(client.revert_session(&base, "/tmp", "s1").await.is_ok());
}

#[tokio::test]
async fn revert_session_no_message_bails() {
    // No user message and no revert pointer -> "No message found to revert".
    let session_body = json!({ "id": "s1" });
    let messages_body = json!([]);
    let base = spawn(revert_router(session_body, messages_body, StatusCode::OK)).await;
    let client = ApiClient::new();
    let err = client
        .revert_session(&base, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("No message found to revert"));
}

#[tokio::test]
async fn revert_session_revert_post_fails() {
    let session_body = json!({ "id": "s1" });
    let messages_body = json!([{ "info": { "role": "user", "id": "u1" } }]);
    let base = spawn(revert_router(
        session_body,
        messages_body,
        StatusCode::INTERNAL_SERVER_ERROR,
    ))
    .await;
    let client = ApiClient::new();
    let err = client
        .revert_session(&base, "/tmp", "s1")
        .await
        .unwrap_err();
    let m = format!("{}", err);
    assert!(m.contains("Session revert rejected") && m.contains("500"));
}

#[tokio::test]
async fn revert_session_messages_not_array_uses_pointer() {
    // messages body is not an array -> as_array() None -> fallback to pointer.
    let session_body = json!({ "revert": { "messageID": "ptr" } });
    let messages_body = json!({ "not": "array" });
    let base = spawn(revert_router(session_body, messages_body, StatusCode::OK)).await;
    let client = ApiClient::new();
    assert!(client.revert_session(&base, "/tmp", "s1").await.is_ok());
}

#[tokio::test]
async fn revert_session_session_fetch_connection_error() {
    let client = ApiClient::new();
    let err = client
        .revert_session(&dead_url().await, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch session for revert"));
}

#[tokio::test]
async fn revert_session_session_parse_error() {
    // /session/{id} returns non-JSON -> parse error.
    let app = Router::new().route("/session/{id}", get(|| async { "not json" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .revert_session(&base, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse session response"));
}

#[tokio::test]
async fn revert_session_messages_parse_error() {
    // session ok, messages endpoint returns non-JSON -> parse error.
    let app = Router::new()
        .route("/session/{id}", get(|| async { Json(json!({})) }))
        .route("/session/{id}/message", get(|| async { "not json" }));
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .revert_session(&base, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse messages response"));
}
