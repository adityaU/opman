//! Claude's own API-error lines carry a structured `limit` when the plan is spent.

use super::*;

fn api_error(text: &str) -> String {
    let line = serde_json::json!({
        "type": "assistant",
        "isApiErrorMessage": true,
        "timestamp": "2026-06-28T08:00:00.000Z",
        "message": { "id": "m1", "content": [{ "type": "text", "text": text }] },
    });
    format!("{line}\n")
}

#[test]
fn usage_limit_api_error_is_tagged() {
    let p = parse_str(&api_error("Claude AI usage limit reached|1760000000"), "ses");
    let info = &p.messages[0].info;
    assert_eq!(info["error"], "Claude AI usage limit reached|1760000000");
    assert_eq!(info["limit"]["kind"], "usage");
    assert_eq!(info["limit"]["resetsAt"], "2025-10-09T08:53:20Z");
}

#[test]
fn hit_your_limit_api_error_is_tagged() {
    let p = parse_str(&api_error("You've hit your limit · resets 3pm (UTC)"), "ses");
    assert_eq!(p.messages[0].info["limit"]["kind"], "usage");
}

#[test]
fn other_api_errors_are_not_tagged() {
    let p = parse_str(&api_error("Overloaded. Please try again."), "ses");
    assert!(p.messages[0].info.get("limit").is_none());
}
