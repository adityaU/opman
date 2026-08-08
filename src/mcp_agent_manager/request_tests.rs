//! Reading the socket line. The wire stays permissive; the accessors do the narrowing.

use super::*;

fn line(json: serde_json::Value) -> ManagerRequest {
    serde_json::from_value(json).expect("a manager request should parse")
}

#[test]
fn an_unknown_operation_is_named_in_the_error() {
    let error = line(serde_json::json!({ "op": "teleport" }))
        .op()
        .expect_err("only four operations exist");

    assert!(format!("{error}").contains("teleport"), "{error}");
}

#[test]
fn the_four_operations_map_to_their_variants() {
    for (text, expected) in [
        ("send", Op::Send),
        ("start", Op::Start),
        ("progress", Op::Progress),
        ("options", Op::Options),
    ] {
        let parsed = line(serde_json::json!({ "op": text }))
            .op()
            .expect("a known operation");
        assert_eq!(parsed, expected, "{text}");
    }
}

/// The bridge forwards whatever the agent passed, so an absent model reaches here and must
/// be refused here rather than dispatched with a hole in it.
#[test]
fn a_send_without_a_model_is_refused_before_it_reaches_a_runner() {
    let request = line(serde_json::json!({ "op": "send", "message": "hi" }));

    assert!(request.dispatch().is_err());
}

#[test]
fn a_send_with_both_halves_produces_a_dispatch() {
    let request = line(serde_json::json!({
        "op": "send",
        "message": "hi",
        "model": "sonnet",
        "effort": "high",
    }));

    let body = request.dispatch().expect("both halves given").body("hi");

    assert_eq!(body["model"]["modelID"], "sonnet");
    assert_eq!(body["effort"], "high");
}

#[test]
fn an_unknown_runner_is_named_in_the_error() {
    let error = line(serde_json::json!({ "op": "send", "runner": "cursor" }))
        .runner_kind()
        .expect_err("unknown runner");

    assert!(format!("{error}").contains("cursor"), "{error}");
}

#[test]
fn an_absent_runner_is_not_an_error() {
    assert!(line(serde_json::json!({ "op": "send" }))
        .runner_kind()
        .expect("absent is legal")
        .is_none());
}

#[test]
fn delivery_aliases_are_supported() {
    let mode = |text: &str| {
        line(serde_json::json!({ "op": "send", "delivery": text }))
            .delivery_mode()
            .unwrap_or_default()
    };

    assert_eq!(mode("steer"), Some(Delivery::Immediate));
    assert_eq!(mode("next_turn"), Some(Delivery::Queued));
    assert_eq!(mode("next-turn"), Some(Delivery::Queued));
    assert!(
        line(serde_json::json!({ "op": "send", "delivery": "later" }))
            .delivery_mode()
            .is_err()
    );
}

/// No `delivery` at all means steer now, which is what a caller expecting a reply wants.
#[test]
fn an_absent_delivery_defaults_to_immediate() {
    assert_eq!(
        line(serde_json::json!({ "op": "send" }))
            .delivery_mode()
            .expect("absent is legal"),
        Some(Delivery::Immediate)
    );
}
