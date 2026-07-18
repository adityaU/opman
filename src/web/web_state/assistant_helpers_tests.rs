//! Generated tests for assistant.rs — the extracted pure mission-loop helpers
//! (`build_send_message_body`, `map_send_status`, `parse_session_id_from_body`,
//! `parse_eval_messages_body`, `apply_verdict`). These had no direct coverage.
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

// ── apply_verdict ───────────────────────────────────────────────────

#[test]
fn apply_verdict_achieved_completes() {
    let (state, iter) = apply_verdict(&EvalVerdict::Achieved, 3, 10);
    assert_eq!(state, MissionState::Completed);
    assert_eq!(iter, 3); // iteration preserved
}

#[test]
fn apply_verdict_failed_fails() {
    let (state, iter) = apply_verdict(&EvalVerdict::Failed, 4, 10);
    assert_eq!(state, MissionState::Failed);
    assert_eq!(iter, 4);
}

#[test]
fn apply_verdict_blocked_pauses() {
    let (state, iter) = apply_verdict(&EvalVerdict::Blocked, 2, 10);
    assert_eq!(state, MissionState::Paused);
    assert_eq!(iter, 2);
}

#[test]
fn apply_verdict_continue_under_max_advances() {
    let (state, iter) = apply_verdict(&EvalVerdict::Continue, 1, 5);
    assert_eq!(state, MissionState::Executing);
    assert_eq!(iter, 2);
}

#[test]
fn apply_verdict_continue_at_max_boundary_advances() {
    // iteration 4, max 5 → next_iter 5, not > 5 → still executing.
    let (state, iter) = apply_verdict(&EvalVerdict::Continue, 4, 5);
    assert_eq!(state, MissionState::Executing);
    assert_eq!(iter, 5);
}

#[test]
fn apply_verdict_continue_over_max_fails() {
    // iteration 5, max 5 → next_iter 6 > 5 → Failed, iteration preserved.
    let (state, iter) = apply_verdict(&EvalVerdict::Continue, 5, 5);
    assert_eq!(state, MissionState::Failed);
    assert_eq!(iter, 5);
}

#[test]
fn apply_verdict_continue_unlimited_never_fails() {
    // max_iterations 0 → infinite → always executing.
    let (state, iter) = apply_verdict(&EvalVerdict::Continue, 999, 0);
    assert_eq!(state, MissionState::Executing);
    assert_eq!(iter, 1000);
}

// ── parse_eval_messages_body ────────────────────────────────────────

fn assistant_msg(created: u64, text: &str) -> serde_json::Value {
    serde_json::json!({
        "info": {
            "role": "assistant",
            "time": { "created": created },
            "parts": [ { "type": "text", "text": text } ]
        }
    })
}

#[test]
fn parse_eval_body_not_array_or_object() {
    // A bare string/number is neither array nor object.
    let r = parse_eval_messages_body(&serde_json::json!("just a string"));
    assert!(matches!(r.verdict, EvalVerdict::Continue));
    assert_eq!(r.summary, "No messages found");

    let r2 = parse_eval_messages_body(&serde_json::json!(42));
    assert_eq!(r2.summary, "No messages found");
}

#[test]
fn parse_eval_body_empty_array_no_assistant() {
    let r = parse_eval_messages_body(&serde_json::json!([]));
    assert!(matches!(r.verdict, EvalVerdict::Continue));
    assert_eq!(r.summary, "No assistant response found");
}

#[test]
fn parse_eval_body_only_user_message() {
    let body = serde_json::json!([
        { "info": { "role": "user", "time": { "created": 1 },
                    "parts": [ { "type": "text", "text": "hi" } ] } }
    ]);
    let r = parse_eval_messages_body(&body);
    assert_eq!(r.summary, "No assistant response found");
}

#[test]
fn parse_eval_body_assistant_empty_text() {
    // assistant present but no text parts/content → empty text.
    let body = serde_json::json!([
        { "info": { "role": "assistant", "time": { "created": 1 } } }
    ]);
    let r = parse_eval_messages_body(&body);
    assert!(matches!(r.verdict, EvalVerdict::Continue));
    assert_eq!(r.summary, "Empty assistant response");
}

#[test]
fn parse_eval_body_valid_json_verdict() {
    let body = serde_json::json!([
        assistant_msg(1, r#"{"verdict":"achieved","summary":"done well"}"#)
    ]);
    let r = parse_eval_messages_body(&body);
    assert!(matches!(r.verdict, EvalVerdict::Achieved));
    assert_eq!(r.summary, "done well");
}

#[test]
fn parse_eval_body_picks_latest_assistant_by_time() {
    // Two assistant messages; the later-created one (200) wins.
    let body = serde_json::json!([
        assistant_msg(100, r#"{"verdict":"failed","summary":"old"}"#),
        assistant_msg(200, r#"{"verdict":"achieved","summary":"new"}"#),
    ]);
    let r = parse_eval_messages_body(&body);
    assert!(matches!(r.verdict, EvalVerdict::Achieved));
    assert_eq!(r.summary, "new");
}

#[test]
fn parse_eval_body_object_keyed_by_id() {
    // Messages as an object keyed by message-id instead of an array.
    let body = serde_json::json!({
        "msg-a": assistant_msg(1, r#"{"verdict":"continue","summary":"a"}"#),
        "msg-b": assistant_msg(9, r#"{"verdict":"blocked","summary":"b"}"#),
    });
    let r = parse_eval_messages_body(&body);
    // msg-b is later → blocked.
    assert!(matches!(r.verdict, EvalVerdict::Blocked));
    assert_eq!(r.summary, "b");
}

#[test]
fn parse_eval_body_missing_time_defaults_zero() {
    // Assistant message with no time → created defaults to 0, still selected
    // as the only assistant message.
    let body = serde_json::json!([
        { "info": { "role": "assistant",
                    "parts": [ { "type": "text", "text": r#"{"verdict":"achieved"}"# } ] } }
    ]);
    let r = parse_eval_messages_body(&body);
    assert!(matches!(r.verdict, EvalVerdict::Achieved));
}

#[test]
fn parse_eval_body_content_fallback_and_heuristic() {
    // Uses /info/content (not parts) and non-JSON text → heuristic path.
    let body = serde_json::json!([
        { "info": { "role": "assistant", "time": { "created": 5 },
                    "content": [ { "type": "text", "text": "The goal has been met." } ] } }
    ]);
    let r = parse_eval_messages_body(&body);
    assert!(matches!(r.verdict, EvalVerdict::Achieved));
}
