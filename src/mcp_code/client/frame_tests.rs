use serde_json::json;

use super::*;

#[test]
fn single_text_block_is_a_string() {
    let out = output_of(&json!({"content": [{"type": "text", "text": "hello"}]})).expect("ok");
    assert_eq!(out.value, json!("hello"));
    assert!(out.attachments.is_empty());
}

#[test]
fn json_text_is_parsed() {
    let out = output_of(&json!({"content": [{"type": "text", "text": "{\"a\": 1}"}]})).expect("ok");
    assert_eq!(out.value, json!({"a": 1}));
}

#[test]
fn json_primitive_results_keep_their_types() {
    for (text, expected) in [
        ("5", json!(5)),
        ("false", json!(false)),
        ("null", Value::Null),
    ] {
        let out = output_of(&json!({"content": [{"type": "text", "text": text}]})).expect("ok");
        assert_eq!(out.value, expected);
    }
}

#[test]
fn broken_json_text_stays_text() {
    let out = output_of(&json!({"content": [{"type": "text", "text": "{nope"}]})).expect("ok");
    assert_eq!(out.value, json!("{nope"));
}

#[test]
fn several_text_blocks_are_joined() {
    let out = output_of(
        &json!({"content": [{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]}),
    )
    .expect("ok");
    assert_eq!(out.value, json!("a\nb"));
}

#[test]
fn structured_content_wins() {
    let out = output_of(&json!({
        "content": [{"type": "text", "text": "ignored"}],
        "structuredContent": {"n": 1}
    }))
    .expect("ok");
    assert_eq!(out.value, json!({"n": 1}));
}

#[test]
fn error_results_become_errors_with_the_tool_text() {
    let err =
        output_of(&json!({"content": [{"type": "text", "text": "bad input"}], "isError": true}))
            .expect_err("error");
    assert_eq!(err, "bad input");
    let err = output_of(&json!({"content": [], "isError": true})).expect_err("error");
    assert!(err.contains("error"));
}

#[test]
fn images_are_attached_not_inlined() {
    let out = output_of(&json!({"content": [
        {"type": "text", "text": "here"},
        {"type": "image", "data": "AAAA", "mimeType": "image/png"}
    ]}))
    .expect("ok");
    assert_eq!(out.attachments.len(), 1);
    assert_eq!(out.attachments[0].mime, "image/png");
    let text = out.value.to_string();
    assert!(
        !text.contains("AAAA"),
        "image data leaked into the value: {text}"
    );
    assert_eq!(out.value[1]["attached"], true);
}

#[test]
fn attachments_are_bounded() {
    let block = json!({"type": "image", "data": "A", "mimeType": "image/png"});
    let out = output_of(&json!({"content": vec![block; 10]})).expect("ok");
    assert_eq!(out.attachments.len(), 4);
    assert_eq!(out.value[9]["attached"], false);
}

#[test]
fn resource_blocks_keep_uri_and_text() {
    let out = output_of(&json!({"content": [
        {"type": "resource", "resource": {"uri": "file:///a", "text": "body"}}
    ]}))
    .expect("ok");
    assert_eq!(out.value[0]["uri"], "file:///a");
    assert_eq!(out.value[0]["text"], "body");
}
