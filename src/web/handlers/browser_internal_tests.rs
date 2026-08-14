use super::*;

fn parse(body: serde_json::Value) -> BrowserCall {
    serde_json::from_value(body).expect("call parses")
}

#[test]
fn the_project_rides_on_the_envelope_not_in_each_arm() {
    let call = parse(serde_json::json!({
        "op": "navigate",
        "project": "/repo",
        "url": "example.com",
    }));
    assert_eq!(call.project, "/repo");
    assert!(matches!(call.operation, Operation::Navigate { url } if url == "example.com"));
}

#[test]
fn an_unnamed_project_is_allowed_and_resolved_later() {
    // A single-project workspace should not have to spell out which one.
    let call = parse(serde_json::json!({ "op": "list" }));
    assert!(call.project.is_empty());
    assert!(matches!(call.operation, Operation::List));
}

#[test]
fn open_without_a_url_connects_rather_than_failing() {
    let call = parse(serde_json::json!({ "op": "open", "project": "/repo" }));
    let Operation::Open { url } = call.operation else {
        panic!("expected an open operation");
    };
    assert!(url.is_none());
}

#[test]
fn snapshot_options_default_to_the_token_budget_when_unspecified() {
    let Operation::Snapshot { options } = parse(serde_json::json!({ "op": "snapshot" })).operation
    else {
        panic!("expected a snapshot operation");
    };
    assert_eq!(options.max_nodes, 400);
    assert_eq!(options.max_chars, 12_000);
}

#[test]
fn snapshot_options_are_overridable_per_call() {
    let Operation::Snapshot { options } = parse(serde_json::json!({
        "op": "snapshot",
        "max_nodes": 40,
        "viewport_only": true,
    }))
    .operation
    else {
        panic!("expected a snapshot operation");
    };
    assert_eq!(options.max_nodes, 40);
    assert!(options.viewport_only);
}

#[test]
fn ref_is_spelled_ref_on_the_wire() {
    // `ref` is a Rust keyword; the field is renamed, and the wire name is what the tool
    // schema promises the model.
    let Operation::Click { reference } =
        parse(serde_json::json!({ "op": "click", "ref": "e12" })).operation
    else {
        panic!("expected a click operation");
    };
    assert_eq!(reference, "e12");
}

#[test]
fn typing_does_not_submit_unless_asked() {
    let Operation::Type { submit, .. } = parse(serde_json::json!({
        "op": "type", "ref": "e3", "text": "hello",
    }))
    .operation
    else {
        panic!("expected a type operation");
    };
    assert!(!submit);
}

#[test]
fn an_unknown_operation_is_rejected_rather_than_ignored() {
    let result: Result<BrowserCall, _> =
        serde_json::from_value(serde_json::json!({ "op": "eval" }));
    assert!(result.is_err(), "`eval` is deliberately not an operation");
}

/// Only operations that can move the page are worth telling the workspace about — a pane
/// that re-revealed itself on every snapshot would fight the user for focus.
#[test]
fn only_page_moving_operations_reveal_a_pane() {
    let moves = |body: serde_json::Value| parse(body).operation.navigates();

    assert!(moves(serde_json::json!({ "op": "open", "url": "x.example" })));
    assert!(moves(serde_json::json!({ "op": "navigate", "url": "x.example" })));
    assert!(moves(serde_json::json!({ "op": "click", "ref": "e1" })));
    assert!(moves(serde_json::json!({ "op": "back" })));

    assert!(!moves(serde_json::json!({ "op": "snapshot" })));
    assert!(!moves(serde_json::json!({ "op": "text" })));
    assert!(!moves(serde_json::json!({ "op": "screenshot" })));
    assert!(!moves(serde_json::json!({ "op": "list" })));
}
