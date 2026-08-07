//! Tests for the pure send helpers in `assistant_send.rs`
//! (`build_send_message_body`, `map_send_status`, `parse_session_id_from_body`,
//! `extract_message_text`).
use super::*;

// ── build_send_message_body ─────────────────────────────────────────

#[test]
fn build_send_message_body_without_model() {
    let body = build_send_message_body("hello world", None);
    assert_eq!(body["parts"][0]["type"], "text");
    assert_eq!(body["parts"][0]["text"], "hello world");
    // No model override object present.
    assert!(body.get("model").is_none());
}

#[test]
fn build_send_message_body_with_model() {
    let model = crate::web::types::ModelRef {
        provider_id: "anthropic".to_string(),
        model_id: "claude-opus".to_string(),
    };
    let body = build_send_message_body("do it", Some(&model));
    assert_eq!(body["parts"][0]["text"], "do it");
    assert_eq!(body["model"]["providerID"], "anthropic");
    assert_eq!(body["model"]["modelID"], "claude-opus");
}

#[test]
fn build_send_message_body_empty_message() {
    let body = build_send_message_body("", None);
    assert_eq!(body["parts"][0]["text"], "");
    assert_eq!(body["parts"].as_array().unwrap().len(), 1);
}

#[test]
fn build_send_message_body_unicode() {
    let body = build_send_message_body("héllo 🚀 世界", None);
    assert_eq!(body["parts"][0]["text"], "héllo 🚀 世界");
}

// ── map_send_status ─────────────────────────────────────────────────

#[test]
fn map_send_status_success_variants_ok() {
    assert!(map_send_status(reqwest::StatusCode::OK).is_ok());
    assert!(map_send_status(reqwest::StatusCode::CREATED).is_ok());
    assert!(map_send_status(reqwest::StatusCode::NO_CONTENT).is_ok());
    assert!(map_send_status(reqwest::StatusCode::ACCEPTED).is_ok());
}

#[test]
fn map_send_status_error_variants_err() {
    let e = map_send_status(reqwest::StatusCode::BAD_REQUEST).unwrap_err();
    assert!(e.contains("Upstream rejected message"));
    assert!(e.contains("400"));

    let e2 = map_send_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR).unwrap_err();
    assert!(e2.contains("500"));

    assert!(map_send_status(reqwest::StatusCode::NOT_FOUND).is_err());
    // 3xx is not "success" per reqwest's is_success (2xx only) → Err.
    assert!(map_send_status(reqwest::StatusCode::FOUND).is_err());
}

// ── parse_session_id_from_body ──────────────────────────────────────

#[test]
fn parse_session_id_present() {
    let body = serde_json::json!({ "id": "sess-abc123", "other": 1 });
    assert_eq!(parse_session_id_from_body(&body).unwrap(), "sess-abc123");
}

#[test]
fn parse_session_id_missing() {
    let body = serde_json::json!({ "notid": "x" });
    let e = parse_session_id_from_body(&body).unwrap_err();
    assert_eq!(e, "No session ID in response");
}

#[test]
fn parse_session_id_wrong_type() {
    // "id" present but not a string → None → Err.
    let body = serde_json::json!({ "id": 42 });
    assert!(parse_session_id_from_body(&body).is_err());
}

#[test]
fn parse_session_id_empty_object() {
    let body = serde_json::json!({});
    assert!(parse_session_id_from_body(&body).is_err());
}

// ── extract_message_text ────────────────────────────────────────────

#[test]
fn extract_message_text_from_parts() {
    let msg = serde_json::json!({
        "info": { "parts": [
            { "type": "text", "text": "one" },
            { "type": "tool", "text": "skipped" },
            { "type": "text", "text": "two" }
        ] }
    });
    assert_eq!(extract_message_text(&msg), "one\ntwo");
}

#[test]
fn extract_message_text_content_fallback() {
    // No parts → falls back to /info/content.
    let msg = serde_json::json!({
        "info": { "content": [ { "type": "text", "text": "The goal has been met." } ] }
    });
    assert_eq!(extract_message_text(&msg), "The goal has been met.");
}

#[test]
fn extract_message_text_empty_parts_falls_through_to_content() {
    // parts present but yields no text → content is consulted.
    let msg = serde_json::json!({
        "info": {
            "parts": [ { "type": "tool", "text": "x" } ],
            "content": [ { "type": "text", "text": "from content" } ]
        }
    });
    assert_eq!(extract_message_text(&msg), "from content");
}

#[test]
fn extract_message_text_missing_everything_is_empty() {
    assert_eq!(extract_message_text(&serde_json::json!({})), "");
    assert_eq!(extract_message_text(&serde_json::json!("bare")), "");
    assert_eq!(
        extract_message_text(&serde_json::json!({ "info": { "role": "assistant" } })),
        ""
    );
}
