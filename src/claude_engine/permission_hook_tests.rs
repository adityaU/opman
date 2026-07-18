//! Coverage for the PreToolUse hook relay (`permission_hook_reply`) — the fail-open
//! branches (no/empty URL, connection error) and the success arms (upstream returns a
//! decision body, or an empty body → allow), driven against a mock `/internal/ask`.
use super::*;
use crate::web::test_support::start_mock_upstream;

/// Parse a decision string and return its `permissionDecision` field, if any.
fn decision(s: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(s)
        .ok()?
        .get("hookSpecificOutput")?
        .get("permissionDecision")?
        .as_str()
        .map(String::from)
}

#[test]
fn allow_decision_shape() {
    assert_eq!(decision(&allow_decision()).as_deref(), Some("allow"));
}

#[tokio::test]
async fn reply_allows_when_url_missing_or_empty() {
    // None → allow.
    let out = permission_hook_reply(serde_json::json!({"tool": "Bash"}), None).await;
    assert_eq!(decision(&out).as_deref(), Some("allow"));
    // Empty string → allow (treated as unset).
    let out = permission_hook_reply(serde_json::json!({}), Some(String::new())).await;
    assert_eq!(decision(&out).as_deref(), Some("allow"));
}

#[tokio::test]
async fn reply_allows_on_connection_error() {
    // A dead loopback port → the relay POST errors → fail-open allow.
    let out = permission_hook_reply(
        serde_json::json!({"tool": "Bash"}),
        Some("http://127.0.0.1:1".to_string()),
    )
    .await;
    assert_eq!(decision(&out).as_deref(), Some("allow"));
}

#[tokio::test]
async fn reply_returns_upstream_body_verbatim() {
    // Mock engine `/internal/ask` returns a concrete decision; the relay passes it
    // through unchanged (this is how a human "reject"/"deny" reaches the agent).
    let mock = axum::Router::new().route(
        "/internal/ask",
        axum::routing::post(|| async {
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}"#
        }),
    );
    let base = start_mock_upstream(mock).await;
    let out = permission_hook_reply(serde_json::json!({"tool": "Write"}), Some(base)).await;
    assert_eq!(decision(&out).as_deref(), Some("deny"));
}

#[tokio::test]
async fn reply_allows_when_upstream_body_empty() {
    // An empty/whitespace body from upstream → fall back to allow.
    let mock = axum::Router::new()
        .route("/internal/ask", axum::routing::post(|| async { "   " }));
    let base = start_mock_upstream(mock).await;
    let out = permission_hook_reply(serde_json::json!({}), Some(base)).await;
    assert_eq!(decision(&out).as_deref(), Some("allow"));
}
