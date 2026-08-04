//! Coverage for jsonl parsing branches not hit by the inline `mod tests`.
use super::*;
use serde_json::json;

// ---- low-level helpers -------------------------------------------------

#[test]
fn iso_to_ms_valid_and_invalid() {
    assert!(iso_to_ms("2026-06-28T08:00:00.000Z") > 0);
    assert_eq!(iso_to_ms("not-a-date"), 0);
    assert_eq!(iso_to_ms(""), 0);
}

#[test]
fn stringify_content_all_shapes() {
    assert_eq!(stringify_content(&json!("plain")), "plain");
    // array of {text} blocks joined with newlines
    assert_eq!(
        stringify_content(&json!([{"type":"text","text":"a"},{"type":"text","text":"b"}])),
        "a\nb"
    );
    // array of bare strings
    assert_eq!(stringify_content(&json!(["x", "y"])), "x\ny");
    // array element that is neither {text} nor string → its JSON repr
    assert_eq!(stringify_content(&json!([{"k":1}])), "{\"k\":1}");
    // a non-string, non-array value → to_string()
    assert_eq!(stringify_content(&json!(42)), "42");
}

#[test]
fn tokens_from_usage_maps_all_fields() {
    let usage = json!({
        "input_tokens": 10,
        "output_tokens": 20,
        "output_tokens_details": { "thinking_tokens": 5 },
        "cache_read_input_tokens": 3,
        "cache_creation_input_tokens": 7,
    });
    let t = tokens_from_usage(&usage);
    assert_eq!(t["input"], 10);
    assert_eq!(t["output"], 20);
    assert_eq!(t["reasoning"], 5);
    assert_eq!(t["cache"]["read"], 3);
    assert_eq!(t["cache"]["write"], 7);
}

#[test]
fn tokens_from_usage_defaults_missing_to_zero() {
    let t = tokens_from_usage(&json!({}));
    assert_eq!(t["input"], 0);
    assert_eq!(t["reasoning"], 0);
    assert_eq!(t["cache"]["write"], 0);
}

#[test]
fn notification_text_prefers_summary() {
    assert_eq!(
        notification_text("junk <summary>the point</summary> more"),
        "the point"
    );
}

#[test]
fn notification_text_truncates_long_and_keeps_short() {
    let long = "x".repeat(400);
    let out = notification_text(&long);
    assert!(out.ends_with('…'));
    assert_eq!(out.chars().count(), 281); // 280 + ellipsis
    assert_eq!(notification_text("  short  "), "short");
    // empty summary falls back to the whole trimmed text
    assert_eq!(
        notification_text("<summary></summary>x"),
        "<summary></summary>x"
    );
}

#[test]
fn is_system_injection_classification() {
    // typed / human origin → genuine prompt
    assert!(!is_system_injection(&json!({"promptSource":"typed"}), "hi"));
    assert!(!is_system_injection(
        &json!({"origin":{"kind":"human"}}),
        "hi"
    ));
    // explicit system source / non-human origin → injection
    assert!(is_system_injection(&json!({"promptSource":"system"}), "hi"));
    assert!(is_system_injection(
        &json!({"origin":{"kind":"tool"}}),
        "hi"
    ));
    // content-sniffed injections when no metadata present
    assert!(is_system_injection(&json!({}), "<task-notification>x"));
    assert!(is_system_injection(&json!({}), "  <system-reminder>x"));
    assert!(is_system_injection(&json!({}), "<local-command-stdout>x"));
    assert!(is_system_injection(&json!({}), "<command-name>x"));
    // plain text → genuine prompt
    assert!(!is_system_injection(&json!({}), "just a normal message"));
}

// ---- parse_str line handling ------------------------------------------

#[test]
fn blank_and_malformed_lines_are_skipped() {
    let transcript = concat!(
        "\n",
        "   \n",
        "not json at all\n",
        "{ \"broken\": \n",
        r#"{"type":"user","promptSource":"typed","timestamp":"2026-06-28T08:00:00.000Z","message":{"role":"user","content":"hello"}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert_eq!(p.messages.len(), 1);
    assert_eq!(p.messages[0].parts[0]["text"], "hello");
}

#[test]
fn ai_title_line_sets_title() {
    let transcript = concat!(
        r#"{"type":"ai-title","aiTitle":"My Session"}"#,
        "\n",
        r#"{"type":"user","promptSource":"typed","message":{"role":"user","content":"hi"}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert_eq!(p.title.as_deref(), Some("My Session"));
}

#[test]
fn typed_user_prompt_becomes_user_bubble() {
    let transcript = concat!(
        r#"{"type":"user","promptSource":"typed","timestamp":"2026-06-28T08:00:00.000Z","message":{"role":"user","content":"do the thing"}}"#,
        "\n",
    );
    let p = parse_str(transcript, "sid");
    assert_eq!(p.messages.len(), 1);
    let m = &p.messages[0];
    assert_eq!(m.info["role"], "user");
    assert_eq!(m.info["id"], "msg_user_sid_1");
    assert_eq!(m.parts[0]["text"], "do the thing");
}

#[test]
fn system_reminder_injection_becomes_system_bubble() {
    let transcript = concat!(
        r#"{"type":"user","timestamp":"2026-06-28T08:00:00.000Z","message":{"role":"user","content":"<system-reminder>be nice</system-reminder>"}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert_eq!(p.messages.len(), 1);
    assert_eq!(p.messages[0].info["role"], "system");
    assert_eq!(p.messages[0].info["variant"], "notification");
    assert_eq!(p.messages[0].info["id"], "msg_sys_ses_1");
}

#[test]
fn assistant_without_message_or_empty_id_is_skipped() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z"}"#,
        "\n",
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:01.000Z","message":{"content":[{"type":"text","text":"x"}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert!(
        p.messages.is_empty(),
        "no id / no message → no assistant message"
    );
}

#[test]
fn thinking_block_becomes_reasoning_part() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"thinking","thinking":"pondering"},{"type":"text","text":"answer"}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let parts = &p.messages[0].parts;
    assert_eq!(parts[0]["type"], "reasoning");
    assert_eq!(parts[0]["text"], "pondering");
    assert_eq!(parts[1]["type"], "text");
}

#[test]
fn model_recorded_once_and_absent_when_missing() {
    // model present on first assistant → recorded; second (no model) does not clobber.
    let with_model = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","model":"claude-opus","content":[{"type":"text","text":"a"}]}}"#,
        "\n",
    );
    let p = parse_str(with_model, "ses");
    assert_eq!(p.model.as_deref(), Some("claude-opus"));

    let no_model = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"text","text":"a"}]}}"#,
        "\n",
    );
    let p = parse_str(no_model, "ses");
    assert!(p.model.is_none());
}

#[test]
fn same_message_id_across_lines_merges_and_refreshes_tokens() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"text","text":"part one"}],"usage":{"input_tokens":1}}}"#,
        "\n",
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:01.000Z","message":{"id":"m1","content":[{"type":"text","text":"part two"}],"usage":{"input_tokens":9}}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert_eq!(
        p.messages.len(),
        1,
        "same message.id merges into one message"
    );
    assert_eq!(p.messages[0].parts.len(), 2);
    // tokens refreshed from the latest usage line
    assert_eq!(p.messages[0].info["tokens"]["input"], 9);
}

#[test]
fn tool_use_without_id_is_not_tracked() {
    // A tool_use missing "id" still renders a part but never registers in tool_loc, so a
    // later matching tool_result can't attach (nothing to attach to).
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let part = &p.messages[0].parts[0];
    assert_eq!(part["tool"], "Bash");
    assert_eq!(part["state"]["status"], "running");
}

#[test]
fn ordinary_tool_result_error_sets_error_state() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"boom"}}]}}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:00:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":[{"type":"text","text":"failed hard"}]}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let s = &p.messages[0].parts[0]["state"];
    assert_eq!(s["status"], "error");
    assert_eq!(s["error"], "failed hard");
    assert_eq!(s["output"], "failed hard");
}

#[test]
fn task_result_without_agent_id_stays_running() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Task","input":{"description":"go"}}]}}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:00:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"launched, no id here"}]}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let part = &p.messages[0].parts[0];
    assert_eq!(part["tool"], "task");
    assert_eq!(part["state"]["status"], "running");
    assert!(part["state"]["metadata"].get("sessionId").is_none());
    assert!(p.subagent_ids.is_empty());
}

#[test]
fn background_launch_ack_error_surfaces_immediately() {
    // A background tool_use whose tool_result is an ERROR that is not a launch ack →
    // the error is surfaced immediately (not left pending for a notification).
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"x","run_in_background":true}}]}}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:00:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"launch blocked by policy"}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let s = &p.messages[0].parts[0]["state"];
    assert_eq!(s["status"], "error");
    assert_eq!(s["error"], "launch blocked by policy");
    assert!(s["time"]["end"].is_u64());
}

#[test]
fn tool_result_for_unknown_id_is_ignored() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}]}}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:00:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"UNKNOWN","content":[{"type":"text","text":"orphan"}]}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    // The original tool part is untouched (still running).
    assert_eq!(p.messages[0].parts[0]["state"]["status"], "running");
}

#[test]
fn api_error_with_no_text_defaults_message() {
    let transcript = concat!(
        r#"{"type":"assistant","isApiErrorMessage":true,"timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"thinking","thinking":"hmm"}]}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert_eq!(p.messages[0].info["error"], "Claude API error");
}

#[test]
fn compact_boundary_without_metadata_uses_defaults() {
    let transcript = concat!(
        r#"{"type":"system","subtype":"compact_boundary","timestamp":"2026-06-28T08:00:00.000Z"}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let card = p
        .messages
        .iter()
        .find(|m| m.info["variant"] == "compact")
        .unwrap();
    assert_eq!(card.info["trigger"], "manual");
    assert_eq!(card.info["preTokens"], 0);
    assert_eq!(card.info["durationMs"], 0);
}

#[test]
fn system_content_whitespace_only_yields_no_bubble() {
    let transcript = concat!(
        r#"{"type":"system","subtype":"informational","content":"   ","timestamp":"2026-06-28T08:00:00.000Z"}"#,
        "\n",
        r#"{"type":"system","subtype":"other-noise","timestamp":"2026-06-28T08:00:01.000Z"}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert!(p.messages.is_empty());
}

#[test]
fn or_insert_completed_is_idempotent_across_reuse() {
    // assistant → turn_duration (completes) → same-id assistant reuses idx → typed user
    // prompt re-marks completed (or_insert no-op). Exercises the already-present branch.
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"text","text":"a"}]}}"#,
        "\n",
        r#"{"type":"system","subtype":"turn_duration","timestamp":"2026-06-28T08:00:01.000Z"}"#,
        "\n",
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:02.000Z","message":{"id":"m1","content":[{"type":"text","text":"b"}]}}"#,
        "\n",
        r#"{"type":"user","promptSource":"typed","timestamp":"2026-06-28T08:00:03.000Z","message":{"role":"user","content":"next"}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    let asst = p
        .messages
        .iter()
        .find(|m| m.info["role"] == "assistant")
        .unwrap();
    // Completed timestamp is the FIRST one stamped (turn_duration), not overwritten.
    assert_eq!(
        asst.info["time"]["completed"],
        iso_to_ms("2026-06-28T08:00:01.000Z")
    );
}
