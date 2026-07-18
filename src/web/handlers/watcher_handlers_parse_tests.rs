//! Coverage tests for the extracted `parse_watcher_messages` parser.
//!
//! The live `get_watcher_messages` handler proxies to opencode (unreachable in
//! tests), but the message-extraction logic is the bulk of the code and is
//! fully exercised here against crafted JSON bodies.

use super::*;

use serde_json::json;

fn texts(entries: &[WatcherMessageEntry]) -> Vec<String> {
    entries.iter().map(|e| e.text.clone()).collect()
}

#[test]
fn parse_array_user_messages_reversed() {
    let body = json!([
        { "info": { "role": "user" }, "parts": [{ "text": "first" }] },
        { "info": { "role": "user" }, "parts": [{ "text": "second" }] },
    ]);
    let out = parse_watcher_messages(&body);
    // Reversed → most recent (second) first.
    assert_eq!(texts(&out), vec!["second".to_string(), "first".to_string()]);
    assert!(out.iter().all(|e| e.role == "user"));
}

#[test]
fn parse_skips_non_user_roles() {
    let body = json!([
        { "info": { "role": "assistant" }, "parts": [{ "text": "bot" }] },
        { "info": { "role": "user" }, "parts": [{ "text": "human" }] },
        { "info": {}, "parts": [{ "text": "no-role" }] },
    ]);
    let out = parse_watcher_messages(&body);
    assert_eq!(texts(&out), vec!["human".to_string()]);
}

#[test]
fn parse_skips_empty_and_whitespace_text() {
    let body = json!([
        { "info": { "role": "user" }, "parts": [
            { "text": "" },
            { "text": "   " },
            { "text": "kept" },
        ]},
    ]);
    let out = parse_watcher_messages(&body);
    assert_eq!(texts(&out), vec!["kept".to_string()]);
}

#[test]
fn parse_multiple_text_parts_in_one_message() {
    let body = json!([
        { "info": { "role": "user" }, "parts": [
            { "text": "a" },
            { "type": "image" },           // no text field → ignored
            { "text": "b" },
        ]},
    ]);
    let out = parse_watcher_messages(&body);
    // The function flattens all text parts then reverses the whole list
    // (most-recent-first), so within one message the parts come back reversed.
    assert_eq!(texts(&out), vec!["b".to_string(), "a".to_string()]);
}

#[test]
fn parse_object_body_values() {
    // Body is an object keyed by message id → values() are collected.
    let body = json!({
        "m1": { "info": { "role": "user" }, "parts": [{ "text": "only" }] },
    });
    let out = parse_watcher_messages(&body);
    assert_eq!(texts(&out), vec!["only".to_string()]);
}

#[test]
fn parse_missing_parts_yields_nothing() {
    let body = json!([
        { "info": { "role": "user" } },                       // no parts
        { "info": { "role": "user" }, "parts": "notarray" },  // parts not an array
    ]);
    let out = parse_watcher_messages(&body);
    assert!(out.is_empty());
}

#[test]
fn parse_non_container_body_is_empty() {
    assert!(parse_watcher_messages(&json!("a string")).is_empty());
    assert!(parse_watcher_messages(&json!(42)).is_empty());
    assert!(parse_watcher_messages(&json!(null)).is_empty());
}

#[test]
fn parse_empty_array_is_empty() {
    assert!(parse_watcher_messages(&json!([])).is_empty());
}
