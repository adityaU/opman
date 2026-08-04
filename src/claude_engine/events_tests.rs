use super::*;
use crate::claude_engine::ClaudeEngine;
use serde_json::json;

fn msg(info: serde_json::Value, parts: Vec<serde_json::Value>) -> MsgOut {
    MsgOut { info, parts }
}

fn text_msg() -> MsgOut {
    msg(
        json!({ "role": "assistant", "id": "msg_1", "time": { "created": 1 } }),
        vec![
            json!({ "type": "text", "id": "msg_1:0", "text": "hello" }),
            json!({ "type": "text", "id": "msg_1:1", "text": "world" }),
        ],
    )
}

#[test]
fn message_hash_is_stable_for_identical_content() {
    let a = text_msg();
    let b = text_msg();
    assert_eq!(message_hash(&a), message_hash(&b));
}

#[test]
fn message_hash_changes_when_info_changes() {
    let a = text_msg();
    let mut b = text_msg();
    b.info["id"] = json!("msg_2");
    assert_ne!(message_hash(&a), message_hash(&b));
}

#[test]
fn message_hash_changes_when_a_part_changes() {
    let a = text_msg();
    let mut b = text_msg();
    b.parts[0]["text"] = json!("HELLO");
    assert_ne!(message_hash(&a), message_hash(&b));
}

#[test]
fn message_hash_is_order_sensitive_across_parts() {
    let a = text_msg();
    let mut b = text_msg();
    b.parts.swap(0, 1);
    assert_ne!(message_hash(&a), message_hash(&b));
}

#[test]
fn message_hash_handles_zero_parts() {
    let a = msg(json!({ "id": "x" }), vec![]);
    let b = msg(json!({ "id": "x" }), vec![]);
    assert_eq!(message_hash(&a), message_hash(&b));
}

// emit_message pushes one `message.updated` followed by one `message.part.updated`
// per part, all scoped to the given directory, carrying the session id + timestamp.
#[test]
fn emit_message_emits_updated_then_one_event_per_part() {
    let engine = ClaudeEngine::new(None, (false, false, false, false));
    let mut rx = engine.subscribe();
    let m = text_msg();

    emit_message(&engine, "/proj", "ses_x", &m, 4242);

    // 1 message.updated + 2 message.part.updated
    let e0 = rx.try_recv().expect("message.updated");
    assert_eq!(e0.directory, "/proj");
    let v0: serde_json::Value = serde_json::from_str(&e0.data).unwrap();
    assert_eq!(v0["type"], "message.updated");
    assert_eq!(v0["properties"]["info"]["id"], "msg_1");

    let e1 = rx.try_recv().expect("part 0");
    let v1: serde_json::Value = serde_json::from_str(&e1.data).unwrap();
    assert_eq!(v1["type"], "message.part.updated");
    assert_eq!(v1["properties"]["sessionID"], "ses_x");
    assert_eq!(v1["properties"]["time"], 4242);
    assert_eq!(v1["properties"]["part"]["text"], "hello");

    let e2 = rx.try_recv().expect("part 1");
    let v2: serde_json::Value = serde_json::from_str(&e2.data).unwrap();
    assert_eq!(v2["properties"]["part"]["text"], "world");

    // Nothing more.
    assert!(rx.try_recv().is_err());
}

#[test]
fn emit_message_with_no_parts_emits_only_the_message() {
    let engine = ClaudeEngine::new(None, (false, false, false, false));
    let mut rx = engine.subscribe();
    let m = msg(json!({ "role": "assistant", "id": "m" }), vec![]);
    emit_message(&engine, "/d", "s", &m, 1);
    let e0 = rx.try_recv().expect("message.updated");
    let v0: serde_json::Value = serde_json::from_str(&e0.data).unwrap();
    assert_eq!(v0["type"], "message.updated");
    assert!(
        rx.try_recv().is_err(),
        "no part events for a partless message"
    );
}
