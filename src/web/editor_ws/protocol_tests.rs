use super::*;
use serde_json::json;

fn frame(op: &str, id: u64, payload: serde_json::Value) -> Vec<u8> {
    rmp_serde::to_vec_named(&json!({ "id": id, "op": op, "payload": payload }))
        .expect("frame encodes")
}

#[test]
fn decodes_a_request() {
    let bytes = frame("hover", 7, json!({ "path": "a.ts", "line": 3 }));
    let request = decode(&bytes).expect("decodes");
    assert_eq!(request.id, 7);
    assert_eq!(request.op, Op::Hover);
    assert_eq!(request.payload["path"], "a.ts");
}

#[test]
fn kebab_case_names_the_ops() {
    for (name, expected) in [
        ("goto", Op::Goto),
        ("create-file", Op::CreateFile),
        ("create-dir", Op::CreateDir),
        ("cancel", Op::Cancel),
    ] {
        let request = decode(&frame(name, 1, json!({}))).expect("decodes");
        assert_eq!(request.op, expected, "{name}");
    }
}

#[test]
fn an_unknown_op_is_a_decode_error() {
    assert!(decode(&frame("teleport", 1, json!({}))).is_err());
}

#[test]
fn a_missing_payload_defaults_rather_than_failing() {
    let bytes = rmp_serde::to_vec_named(&json!({ "id": 2, "op": "diagnostics" })).expect("encodes");
    let request = decode(&bytes).expect("decodes");
    assert!(request.payload.is_null());
}

#[test]
fn writes_are_not_cancellable() {
    for op in [Op::Write, Op::Delete, Op::Move, Op::Rename, Op::Format, Op::CreateFile, Op::CreateDir] {
        assert!(!op.is_read_only(), "{op:?} must not be abandoned mid-flight");
    }
    for op in [Op::Hover, Op::Goto, Op::References, Op::Completion, Op::Diagnostics, Op::Browse, Op::Read] {
        assert!(op.is_read_only(), "{op:?} should be cancellable");
    }
}

#[test]
fn a_success_carries_no_error_key() {
    let bytes = encode(&Response::ok(3, json!({ "hover": "x" }))).expect("encodes");
    let value: serde_json::Value = rmp_serde::from_slice(&bytes).expect("decodes");
    assert_eq!(value["id"], 3);
    assert_eq!(value["result"]["hover"], "x");
    assert!(value.get("error").is_none());
}

#[test]
fn a_null_result_is_still_a_success() {
    let bytes = encode(&Response::ok(4, json!(null))).expect("encodes");
    let value: serde_json::Value = rmp_serde::from_slice(&bytes).expect("decodes");
    assert!(value.get("error").is_none());
    assert!(value["result"].is_null());
}

#[test]
fn a_failure_carries_no_result_key() {
    let bytes = encode(&Response::failed(5, "nope")).expect("encodes");
    let value: serde_json::Value = rmp_serde::from_slice(&bytes).expect("decodes");
    assert_eq!(value["error"], "nope");
    assert!(value.get("result").is_none());
}

#[test]
fn events_are_marked_by_a_zero_id() {
    let bytes = encode(&Event::new("diagnostics", json!([]))).expect("encodes");
    let value: serde_json::Value = rmp_serde::from_slice(&bytes).expect("decodes");
    assert_eq!(value["id"], 0);
    assert_eq!(value["event"], "diagnostics");
}

/// The point of the binary framing: a buffer rides as bytes, not as an escaped
/// JSON string.
#[test]
fn a_buffer_is_smaller_than_its_json_form() {
    let text = "const answer = 42;\n\"quoted\"\n\ttabbed\n".repeat(400);
    let payload = json!({ "path": "big.ts", "content": text });
    let packed = rmp_serde::to_vec_named(&payload).expect("packs");
    let json = serde_json::to_vec(&payload).expect("serialises");
    assert!(packed.len() < json.len(), "{} vs {}", packed.len(), json.len());
}
