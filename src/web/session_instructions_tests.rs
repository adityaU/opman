use super::*;
use crate::web::types::MemoryScope;

fn item(label: &str, content: &str) -> PersonalMemoryItem {
    PersonalMemoryItem {
        id: format!("memory-{label}"),
        label: label.to_string(),
        content: content.to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn wrap_puts_instructions_above_the_users_own_text() {
    let out = wrap("ship it", &[item("tone", "be terse")]);
    assert!(out.starts_with("[Session instructions]\n"));
    assert!(out.contains("- tone: be terse"));
    assert!(out.ends_with("[User request]\nship it"));
}

#[test]
fn wrap_is_a_no_op_without_instructions() {
    assert_eq!(wrap("ship it", &[]), "ship it");
}

/// A queued or retried message can arrive already wrapped. Wrapping again would
/// duplicate the block and bill for it twice.
#[test]
fn wrap_refuses_to_nest_a_second_block() {
    let once = wrap("ship it", &[item("tone", "be terse")]);
    let twice = wrap(&once, &[item("tone", "be terse")]);
    assert_eq!(once, twice);
}

#[test]
fn wrap_recognises_the_pre_rename_marker() {
    let legacy = format!("{LEGACY_MARKER}\n- tone: be terse\n{REQUEST_MARKER}\nship it");
    assert_eq!(wrap(&legacy, &[item("tone", "be terse")]), legacy);
    assert!(carries_block(&legacy));
}

#[test]
fn apply_to_parts_prefixes_only_the_first_text_part() {
    let mut parts = vec![
        serde_json::json!({ "type": "text", "text": "first" }),
        serde_json::json!({ "type": "text", "text": "second" }),
    ];
    assert!(apply_to_parts(&mut parts, &[item("tone", "be terse")]));
    assert!(parts[0]["text"]
        .as_str()
        .unwrap()
        .starts_with(INSTRUCTIONS_MARKER));
    assert_eq!(parts[1]["text"], "second");
}

/// Images and files carry no prose; the instructions must skip past them to the
/// text part rather than landing on an attachment.
#[test]
fn apply_to_parts_skips_non_text_parts() {
    let mut parts = vec![
        serde_json::json!({ "type": "file", "url": "data:image/png;base64,AA", "mime": "image/png" }),
        serde_json::json!({ "type": "text", "text": "look at this" }),
    ];
    assert!(apply_to_parts(&mut parts, &[item("tone", "be terse")]));
    assert!(parts[0].get("text").is_none());
    assert!(parts[1]["text"].as_str().unwrap().contains("look at this"));
}

#[test]
fn apply_to_parts_reports_nothing_written_when_already_wrapped() {
    let wrapped = wrap("ship it", &[item("tone", "be terse")]);
    let mut parts = vec![serde_json::json!({ "type": "text", "text": wrapped })];
    assert!(!apply_to_parts(&mut parts, &[item("tone", "be terse")]));
}

#[test]
fn apply_to_parts_reports_nothing_written_without_instructions() {
    let mut parts = vec![serde_json::json!({ "type": "text", "text": "ship it" })];
    assert!(!apply_to_parts(&mut parts, &[]));
    assert_eq!(parts[0]["text"], "ship it");
}
