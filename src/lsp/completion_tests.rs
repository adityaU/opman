//! Completion conversion tests.
//!
//! Every case here is one where being wrong inserts the *wrong text* into the
//! user's file — a failure mode that looks like the feature working.

use super::*;
use serde_json::json;

fn item(extra: Value) -> Value {
    let mut base = json!({ "label": "push", "kind": 2 });
    if let (Some(target), Some(src)) = (base.as_object_mut(), extra.as_object()) {
        for (key, value) in src {
            target.insert(key.clone(), value.clone());
        }
    }
    base
}

// ── Result shapes ───────────────────────────────────────

#[test]
fn accepts_a_bare_array() {
    let (items, incomplete) = render_result(&json!([item(json!({}))]));
    assert_eq!(items.len(), 1);
    assert!(!incomplete);
}

#[test]
fn accepts_a_completion_list() {
    let (items, incomplete) = render_result(&json!({
        "isIncomplete": true,
        "items": [item(json!({}))]
    }));
    assert_eq!(items.len(), 1);
    assert!(
        incomplete,
        "an incomplete list must be re-queried, not filtered"
    );
}

#[test]
fn a_null_result_is_empty() {
    let (items, _) = render_result(&Value::Null);
    assert!(items.is_empty());
}

/// rust-analyzer will return thousands for an empty prefix; the cap must also
/// report the list as incomplete so the editor keeps re-querying.
#[test]
fn oversized_lists_are_capped_and_marked_incomplete() {
    let many: Vec<Value> = (0..500).map(|_| item(json!({}))).collect();
    let (items, incomplete) = render_result(&json!(many));
    assert_eq!(items.len(), MAX_ITEMS);
    assert!(incomplete);
}

// ── Insertion text ──────────────────────────────────────

#[test]
fn falls_back_to_the_label() {
    let (items, _) = render_result(&json!([item(json!({}))]));
    assert_eq!(items[0]["insert"], "push");
    assert_eq!(items[0]["snippet"], false);
}

#[test]
fn insert_text_wins_over_the_label() {
    let (items, _) = render_result(&json!([item(json!({ "insertText": "push_str" }))]));
    assert_eq!(items[0]["insert"], "push_str");
}

/// `textEdit` is the only form that can replace more than the typed word, so
/// it must take precedence over `insertText`.
#[test]
fn text_edit_wins_over_insert_text() {
    let (items, _) = render_result(&json!([item(json!({
        "insertText": "wrong",
        "textEdit": {
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 4 } },
            "newText": "right"
        }
    }))]));
    assert_eq!(items[0]["insert"], "right");
}

// ── Snippets ────────────────────────────────────────────

#[test]
fn numbered_placeholders_become_codemirror_fields() {
    assert_eq!(snippet_to_codemirror("println!($0)"), "println!(${})");
    assert_eq!(
        snippet_to_codemirror("match ${1:expr} { $0 }"),
        "match ${expr} { ${} }"
    );
    assert_eq!(snippet_to_codemirror("${1}"), "${}");
}

/// Per the spec `$5` really is tabstop 5, so a literal dollar has to be
/// escaped. A `$` followed by anything else stays a dollar sign.
#[test]
fn a_dollar_before_a_digit_is_a_tabstop() {
    assert_eq!(snippet_to_codemirror("cost is $5"), "cost is ${}");
    assert_eq!(snippet_to_codemirror("cost is \\$5"), "cost is $5");
    assert_eq!(snippet_to_codemirror("100% of $x"), "100% of $x");
    assert_eq!(snippet_to_codemirror("trailing $"), "trailing $");
}

#[test]
fn escapes_are_preserved() {
    assert_eq!(snippet_to_codemirror("\\$notafield"), "$notafield");
    assert_eq!(snippet_to_codemirror("a\\}b"), "a}b");
}

/// CodeMirror treats `#{}` as a field too, so a literal one from the server
/// must be escaped or it silently becomes an editable hole.
#[test]
fn literal_hash_brace_is_escaped() {
    assert_eq!(snippet_to_codemirror("#{not a field}"), "\\#{not a field}");
}

#[test]
fn snippet_items_are_flagged_and_translated() {
    let (items, _) = render_result(&json!([item(json!({
        "insertText": "push(${1:value})$0",
        "insertTextFormat": 2
    }))]));
    assert_eq!(items[0]["snippet"], true);
    assert_eq!(items[0]["insert"], "push(${value})${}");
}

/// Format 1 is plain text — a `$` in it is a dollar sign, not a field.
#[test]
fn plain_text_items_are_not_translated() {
    let (items, _) = render_result(&json!([item(json!({
        "insertText": "price$1",
        "insertTextFormat": 1
    }))]));
    assert_eq!(items[0]["snippet"], false);
    assert_eq!(items[0]["insert"], "price$1");
}

// ── Metadata ────────────────────────────────────────────

#[test]
fn kinds_map_to_names() {
    let (items, _) = render_result(&json!([
        item(json!({ "kind": 2 })),
        item(json!({ "kind": 6 })),
        item(json!({ "kind": 15 })),
        item(json!({ "kind": 999 })),
    ]));
    assert_eq!(items[0]["kind"], "method");
    assert_eq!(items[1]["kind"], "variable");
    assert_eq!(items[2]["kind"], "snippet");
    assert_eq!(items[3]["kind"], "text", "unknown kinds degrade, not crash");
}

#[test]
fn label_details_are_preferred_for_the_signature() {
    let (items, _) = render_result(&json!([item(json!({
        "detail": "fallback",
        "labelDetails": { "detail": "(&mut self, value: T)" }
    }))]));
    assert_eq!(items[0]["detail"], "(&mut self, value: T)");
}

#[test]
fn markup_documentation_is_flattened() {
    let (items, _) = render_result(&json!([item(json!({
        "documentation": { "kind": "markdown", "value": "Appends an element." }
    }))]));
    assert_eq!(items[0]["documentation"], "Appends an element.");
}

/// The server's own ordering is better than anything we could compute, so
/// `sortText` must survive to the client.
#[test]
fn sort_text_is_carried_through() {
    let (items, _) = render_result(&json!([item(json!({ "sortText": "0001" }))]));
    assert_eq!(items[0]["sort"], "0001");

    let (fallback, _) = render_result(&json!([item(json!({}))]));
    assert_eq!(fallback[0]["sort"], "push", "falls back to the label");
}

#[test]
fn deprecation_is_read_from_either_spelling() {
    let (old, _) = render_result(&json!([item(json!({ "deprecated": true }))]));
    assert_eq!(old[0]["deprecated"], true);

    let (tagged, _) = render_result(&json!([item(json!({ "tags": [1] }))]));
    assert_eq!(tagged[0]["deprecated"], true);

    let (plain, _) = render_result(&json!([item(json!({}))]));
    assert_eq!(plain[0]["deprecated"], false);
}

#[test]
fn items_without_a_label_are_dropped() {
    let (items, _) = render_result(&json!([json!({ "kind": 2 })]));
    assert!(items.is_empty());
}
