//! Generated coverage tests (wave 2) for `search_handlers.rs`.
//!
//! Focus: the `collect_session_matches` parser extracted from the
//! opencode-proxy `search_messages` loop (the `.send().await` upstream call
//! is unreachable in tests, so the matching/snippet logic is exercised here
//! against crafted JSON bodies).

use super::*;

use crate::web::types::SearchResultEntry;
use serde_json::json;

fn run(body: serde_json::Value, needle: &str, limit: usize) -> Vec<SearchResultEntry> {
    let mut results = Vec::new();
    collect_session_matches(&body, "sid", "My Session", "Proj", needle, limit, &mut results);
    results
}

// ── body-shape normalisation ────────────────────────────────────────

#[test]
fn collect_body_not_array_or_object_returns_nothing() {
    assert!(run(json!("a string"), "a", 50).is_empty());
    assert!(run(json!(42), "4", 50).is_empty());
    assert!(run(json!(null), "x", 50).is_empty());
}

#[test]
fn collect_object_body_uses_values() {
    // Map/object body → obj.values() path.
    let body = json!({
        "m1": { "info": { "role": "user" }, "parts": [ { "text": "find the needle here" } ] }
    });
    let r = run(body, "needle", 50);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].role, "user");
}

// ── field population ────────────────────────────────────────────────

#[test]
fn collect_populates_all_fields() {
    let body = json!([
        {
            "info": { "role": "assistant", "id": "msg-1", "time": { "created": 1234 } },
            "parts": [ { "text": "hello NEEDLE world" } ]
        }
    ]);
    let r = run(body, "needle", 50);
    assert_eq!(r.len(), 1);
    let e = &r[0];
    assert_eq!(e.session_id, "sid");
    assert_eq!(e.session_title, "My Session");
    assert_eq!(e.project_name, "Proj");
    assert_eq!(e.message_id, "msg-1");
    assert_eq!(e.role, "assistant");
    assert_eq!(e.timestamp, 1234);
    assert!(e.snippet.to_lowercase().contains("needle"));
}

#[test]
fn collect_message_id_falls_back_to_messageID() {
    let body = json!([
        { "info": { "messageID": "alt-id" }, "parts": [ { "text": "needle" } ] }
    ]);
    let r = run(body, "needle", 50);
    assert_eq!(r[0].message_id, "alt-id");
}

#[test]
fn collect_missing_info_uses_defaults() {
    // No /info → role "unknown", id "", timestamp 0.
    let body = json!([ { "parts": [ { "text": "the needle" } ] } ]);
    let r = run(body, "needle", 50);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].role, "unknown");
    assert_eq!(r[0].message_id, "");
    assert_eq!(r[0].timestamp, 0);
}

// ── each searchable part field ──────────────────────────────────────

#[test]
fn collect_matches_tool_name() {
    let body = json!([ { "parts": [ { "toolName": "bash_needle" } ] } ]);
    assert_eq!(run(body, "needle", 50).len(), 1);
}

#[test]
fn collect_matches_args_string() {
    let body = json!([ { "parts": [ { "args": "cmd --needle" } ] } ]);
    assert_eq!(run(body, "needle", 50).len(), 1);
}

#[test]
fn collect_ignores_non_string_args() {
    // args present but not a string → not pushed, no match, no panic.
    let body = json!([ { "parts": [ { "args": { "k": "needle" } } ] } ]);
    assert!(run(body, "needle", 50).is_empty());
}

#[test]
fn collect_matches_output() {
    let body = json!([ { "parts": [ { "output": "result: needle" } ] } ]);
    assert_eq!(run(body, "needle", 50).len(), 1);
}

#[test]
fn collect_matches_result() {
    let body = json!([ { "parts": [ { "result": "the needle output" } ] } ]);
    assert_eq!(run(body, "needle", 50).len(), 1);
}

// ── no-match and empty parts ────────────────────────────────────────

#[test]
fn collect_no_match_returns_empty() {
    let body = json!([ { "parts": [ { "text": "nothing relevant" } ] } ]);
    assert!(run(body, "needle", 50).is_empty());
}

#[test]
fn collect_message_without_parts_array() {
    // parts missing / not an array → skipped, no result.
    let body = json!([ { "info": { "role": "user" } }, { "parts": "notarray" } ]);
    assert!(run(body, "needle", 50).is_empty());
}

#[test]
fn collect_part_with_no_searchable_fields() {
    let body = json!([ { "parts": [ { "type": "step-start" } ] } ]);
    assert!(run(body, "needle", 50).is_empty());
}

// ── one match per message ───────────────────────────────────────────

#[test]
fn collect_one_result_per_matching_part_breaks_texts() {
    // Both text and toolName contain the needle in one part → still one push.
    let body = json!([ { "parts": [ { "text": "needle a", "toolName": "needle_tool" } ] } ]);
    assert_eq!(run(body, "needle", 50).len(), 1);
}

// ── limit handling ──────────────────────────────────────────────────

#[test]
fn collect_limit_stops_before_message_loop() {
    // results already at limit → immediate break, no additions.
    let mut results = vec![SearchResultEntry {
        session_id: "x".into(),
        session_title: "x".into(),
        project_name: "x".into(),
        message_id: "x".into(),
        role: "user".into(),
        snippet: "x".into(),
        timestamp: 0,
    }];
    let body = json!([ { "parts": [ { "text": "needle" } ] } ]);
    collect_session_matches(&body, "s", "t", "p", "needle", 1, &mut results);
    assert_eq!(results.len(), 1);
}

#[test]
fn collect_limit_stops_across_messages() {
    let body = json!([
        { "parts": [ { "text": "needle 1" } ] },
        { "parts": [ { "text": "needle 2" } ] },
        { "parts": [ { "text": "needle 3" } ] }
    ]);
    let r = run(body, "needle", 2);
    assert_eq!(r.len(), 2);
}

#[test]
fn collect_limit_stops_within_parts_of_one_message() {
    // One message with many matching parts, limit 2 → break inside parts loop.
    let body = json!([
        { "parts": [
            { "text": "needle a" },
            { "text": "needle b" },
            { "text": "needle c" }
        ] }
    ]);
    let r = run(body, "needle", 2);
    assert_eq!(r.len(), 2);
}
