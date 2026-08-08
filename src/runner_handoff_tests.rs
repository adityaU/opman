use super::*;
use serde_json::json;

fn message(role: &str, text: &str, created: u64) -> Value {
    json!({
        "info": { "role": role, "time": { "created": created } },
        "parts": [{ "type": "text", "text": text }],
    })
}

#[test]
fn renders_turns_in_creation_order_from_a_map() {
    let body = json!({
        "b": message("assistant", "second", 2),
        "a": message("user", "first", 1),
    });
    let out = render_transcript(&body);
    assert_eq!(out, "--- user ---\nfirst\n\n--- assistant ---\nsecond");
}

#[test]
fn empty_transcript_says_so() {
    assert_eq!(
        render_transcript(&json!([])),
        "No transcript was available."
    );
}

#[test]
fn a_previous_handoff_block_is_not_quoted_again() {
    let earlier = build_prompt("claude-code", "--- user ---\nold work", "please continue");
    let body = json!([message("user", &earlier, 1)]);
    let out = render_transcript(&body);
    assert_eq!(out, "--- user ---\nplease continue");
    assert!(!out.contains(HANDOFF_MARKER));
    assert!(!out.contains("old work"));
}

#[test]
fn session_instructions_are_stripped_from_history() {
    let wrapped = format!("{INSTRUCTIONS_MARKER}\n- Tone: terse\n\n{REQUEST_MARKER}\nship it");
    let body = json!([message("user", &wrapped, 1)]);
    assert_eq!(render_transcript(&body), "--- user ---\nship it");
}

#[test]
fn a_handoff_wrapping_instructions_keeps_only_the_user_text() {
    let inner = format!("{INSTRUCTIONS_MARKER}\n- Tone: terse\n\n{REQUEST_MARKER}\nship it");
    let earlier = build_prompt("codex", "--- user ---\nold", &inner);
    let body = json!([message("user", &earlier, 1)]);
    assert_eq!(render_transcript(&body), "--- user ---\nship it");
}

#[test]
fn oldest_turns_are_dropped_first_when_over_budget() {
    let long = "x".repeat(30_000);
    let body = json!([
        message("user", &long, 1),
        message("assistant", "the recent answer", 2),
    ]);
    let out = render_transcript(&body);
    assert!(out.starts_with(TRUNCATION_NOTICE));
    assert!(out.contains("the recent answer"));
    assert!(out.len() <= MAX_TRANSCRIPT + TRUNCATION_NOTICE.len() + 2);
}

#[test]
fn prompt_puts_user_text_after_the_end_marker() {
    let out = build_prompt("opencode", "--- user ---\nhi", "now do the thing");
    let (_, tail) = out.split_once(HANDOFF_END_MARKER).expect("end marker");
    assert_eq!(tail.trim(), "now do the thing");
    assert!(out.starts_with(HANDOFF_MARKER));
    assert!(out.contains("opencode"));
}

#[test]
fn messages_without_text_are_skipped() {
    let body = json!([
        json!({ "info": { "role": "assistant", "time": { "created": 1 } }, "parts": [{ "type": "tool" }] }),
        message("user", "   ", 2),
        message("assistant", "kept", 3),
    ]);
    assert_eq!(render_transcript(&body), "--- assistant ---\nkept");
}

#[test]
fn a_single_oversized_turn_keeps_its_tail() {
    let huge = format!("{}THE ASK", "y".repeat(30_000));
    let body = json!([message("user", &huge, 1)]);
    let out = render_transcript(&body);
    assert!(out.starts_with(TRUNCATION_NOTICE));
    assert!(out.ends_with("THE ASK"));
    assert!(out.len() < 30_000);
}

#[test]
fn multibyte_text_does_not_panic_when_truncated() {
    let huge = "é".repeat(20_000);
    let body = json!([message("user", &huge, 1)]);
    assert!(!render_transcript(&body).is_empty());
}
