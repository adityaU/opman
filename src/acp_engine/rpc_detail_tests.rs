use super::detail;
use serde_json::json;

#[test]
fn details_field_wins() {
    let error = json!({ "message": "m", "data": { "details": "why", "x": 1 } });
    assert_eq!(detail(&error).as_deref(), Some("why"));
}

#[test]
fn a_string_is_kept_verbatim() {
    let error = json!({ "message": "m", "data": "plain reason" });
    assert_eq!(detail(&error).as_deref(), Some("plain reason"));
}

#[test]
fn other_data_becomes_json() {
    let error = json!({ "message": "m", "data": { "code": "E_AUTH" } });
    assert_eq!(detail(&error).as_deref(), Some(r#"{"code":"E_AUTH"}"#));
}

#[test]
fn no_data_is_none() {
    assert_eq!(detail(&json!({ "message": "m" })), None);
    assert_eq!(detail(&json!({ "message": "m", "data": null })), None);
}
