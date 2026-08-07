//! The pure parts of the upstream client.

use super::*;

#[test]
fn a_bearer_challenge_yields_its_scope() {
    assert_eq!(
        parse_scope(r#"Bearer error="insufficient_scope", scope="files:write""#),
        Some("files:write".to_string())
    );
}

#[test]
fn an_unquoted_scope_is_accepted() {
    assert_eq!(
        parse_scope("Bearer scope=files:read, realm=x"),
        Some("files:read".to_string())
    );
}

#[test]
fn a_challenge_without_a_scope_yields_none() {
    assert!(parse_scope(r#"Bearer error="invalid_token""#).is_none());
    assert!(parse_scope("").is_none());
}

#[test]
fn sse_data_lines_decode_to_messages() {
    let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1}\n\n";
    let values = decode_sse(body);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["id"], 1);
}

/// A server may split one payload across several `data:` lines; they fold into one
/// message. Field lines start at column 0 — a leading space would be part of the value.
#[test]
fn multi_line_data_is_folded_into_one_message() {
    let body = "data: {\"jsonrpc\":\ndata: \"2.0\",\"id\":7}\n\n";
    let values = decode_sse(body);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["id"], 7);
}

#[test]
fn several_events_decode_in_order() {
    let body = "data: {\"id\":1}\n\ndata: {\"id\":2}\n\n";
    let values = decode_sse(body);
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["id"], 1);
    assert_eq!(values[1]["id"], 2);
}

#[test]
fn crlf_line_endings_decode() {
    let body = "data: {\"id\":3}\r\n\r\n";
    assert_eq!(decode_sse(body)[0]["id"], 3);
}

#[test]
fn comment_and_event_lines_are_ignored() {
    let body = ": keepalive\nevent: message\ndata: {\"id\":4}\n\n";
    assert_eq!(decode_sse(body)[0]["id"], 4);
}
