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

// ---- send_session_message (fire-and-forget) -----------------------------

#[tokio::test]
async fn send_session_message_success() {
    let app = Router::new().route(
        "/session/{id}/message",
        post(|Json(_b): Json<Value>| async { Json(json!({})) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client
        .send_session_message(&base, "/tmp", "s1", "hello")
        .await
        .is_ok());
}

#[tokio::test]
async fn send_session_message_rejects_error_status() {
    let app = Router::new().route(
        "/session/{id}/message",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .send_session_message(&base, "/tmp", "s1", "hi")
        .await
        .unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("500") && message.contains("boom"));
}

#[tokio::test]
async fn send_session_message_connection_error() {
    let client = ApiClient::new();
    let err = client
        .send_session_message(&dead_url().await, "/tmp", "s1", "hi")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to send message to opencode session"));
}

// ---- send_system_message_async ------------------------------------------

#[tokio::test]
async fn send_system_message_async_success() {
    let app = Router::new().route(
        "/session/{id}/prompt_async",
        post(|Json(_b): Json<Value>| async { Json(json!({})) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    assert!(client
        .send_system_message_async(&base, "/tmp", "s1", "sys")
        .await
        .is_ok());
}

#[tokio::test]
async fn send_system_message_async_failure_status() {
    let app = Router::new().route(
        "/session/{id}/prompt_async",
        post(|| async { (StatusCode::BAD_REQUEST, "rejected".to_string()) }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .send_system_message_async(&base, "/tmp", "s1", "sys")
        .await
        .unwrap_err();
    let m = format!("{}", err);
    assert!(m.contains("400") && m.contains("rejected"));
}

#[tokio::test]
async fn send_system_message_async_connection_error() {
    let client = ApiClient::new();
    let err = client
        .send_system_message_async(&dead_url().await, "/tmp", "s1", "sys")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to send async system message"));
}

// ---- fetch_session_messages ---------------------------------------------

/// Build a server that returns the given canned JSON for GET /session/{id}/message.
async fn msg_server(body: Value) -> String {
    let app = Router::new().route(
        "/session/{id}/message",
        get(move || {
            let b = body.clone();
            async move { Json(b) }
        }),
    );
    spawn(app).await
}

#[tokio::test]
async fn fetch_messages_array_info_role_and_parts() {
    let body = json!([
        {
            "info": { "role": "user" },
            "parts": [
                { "type": "text", "text": "line1" },
                { "type": "text", "text": "line2" },
                { "type": "tool", "text": "ignored" }
            ]
        },
        { "info": { "role": "assistant" }, "parts": [{ "type": "text", "text": "skip me" }] }
    ]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].role, "user");
    assert_eq!(msgs[0].text, "line1\nline2");
}

#[tokio::test]
async fn fetch_messages_object_map_form() {
    // Response as object keyed by message id.
    let body = json!({
        "m1": { "info": { "role": "user" }, "parts": [{ "type": "text", "text": "hey" }] }
    });
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text, "hey");
}

#[tokio::test]
async fn fetch_messages_top_level_role_fallback() {
    // No info.role -> falls back to top-level role. Empty-type part counts as text.
    let body = json!([
        { "role": "user", "parts": [{ "text": "notype" }] }
    ]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].text, "notype");
}

#[tokio::test]
async fn fetch_messages_text_field_fallback() {
    // No parts array -> uses top-level "text".
    let body = json!([
        { "info": { "role": "user" }, "text": "plain-text" }
    ]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert_eq!(msgs[0].text, "plain-text");
}

#[tokio::test]
async fn fetch_messages_content_field_fallback() {
    // No parts, no text -> uses "content".
    let body = json!([
        { "info": { "role": "user" }, "content": "content-text" }
    ]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert_eq!(msgs[0].text, "content-text");
}

#[tokio::test]
async fn fetch_messages_skip_when_no_text_source() {
    // user role but no parts/text/content -> continue (skipped).
    let body = json!([
        { "info": { "role": "user" } }
    ]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn fetch_messages_skip_empty_text() {
    // parts produce empty joined string -> skipped by is_empty guard.
    let body = json!([
        { "info": { "role": "user" }, "parts": [] }
    ]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn fetch_messages_non_array_non_object_yields_empty() {
    let base = msg_server(json!("just a string")).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn fetch_messages_missing_role_defaults_non_user() {
    // No role at all -> "" != "user" -> skipped.
    let body = json!([{ "parts": [{ "type": "text", "text": "x" }] }]);
    let base = msg_server(body).await;
    let client = ApiClient::new();
    let msgs = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap();
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn fetch_messages_malformed_json_errors() {
    let app = Router::new().route(
        "/session/{id}/message",
        get(|| async { (StatusCode::OK, "not json at all") }),
    );
    let base = spawn(app).await;
    let client = ApiClient::new();
    let err = client
        .fetch_session_messages(&base, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to parse session messages response"));
}

#[tokio::test]
async fn fetch_messages_connection_error() {
    let client = ApiClient::new();
    let err = client
        .fetch_session_messages(&dead_url().await, "/tmp", "s1")
        .await
        .unwrap_err();
    assert!(format!("{}", err).contains("Failed to fetch session messages"));
}
