use axum::body::Bytes;

use super::sse_parse::{SseEvent, SseParser};

fn parse(chunks: &[&[u8]]) -> Vec<SseEvent> {
    let mut parser = SseParser::default();
    let mut events = Vec::new();
    for chunk in chunks {
        parser.push(Bytes::copy_from_slice(chunk));
        while let Some(event) = parser.next_event() {
            events.push(event);
        }
    }
    events
}

fn ev(event: &str, data: &str, last_id: Option<&str>) -> SseEvent {
    SseEvent {
        event: event.into(),
        data: data.into(),
        last_id: last_id.map(Into::into),
    }
}

#[test]
fn named_and_default_events() {
    let events = parse(&[b"event: output\ndata: abc\n\ndata: plain\n\n"]);
    assert_eq!(events, vec![ev("output", "abc", None), ev("message", "plain", None)]);
}

#[test]
fn multi_line_data_joins_with_newlines() {
    let events = parse(&[b"data: one\ndata:two\ndata:  three\n\n"]);
    assert_eq!(events, vec![ev("message", "one\ntwo\n three", None)]);
}

#[test]
fn comments_keepalives_and_unknown_fields_are_ignored() {
    let events = parse(&[b": keep-alive\n\nretry: 10\nfoo\n:x\ndata: y\n\n"]);
    assert_eq!(events, vec![ev("message", "y", None)]);
}

#[test]
fn an_event_without_data_is_dropped_but_empty_data_is_kept() {
    let events = parse(&[b"event: nothing\n\nevent: state_changed\ndata\n\n"]);
    assert_eq!(events, vec![ev("state_changed", "", None)]);
}

#[test]
fn ids_stick_until_changed() {
    let events = parse(&[b"id: 7\ndata: a\n\ndata: b\n\nid: 8\ndata: c\n\n"]);
    assert_eq!(
        events,
        vec![ev("message", "a", Some("7")), ev("message", "b", Some("7")), ev("message", "c", Some("8"))]
    );
}

#[test]
fn every_line_ending_works() {
    let events = parse(&[b"data: a\r\n\r\ndata: b\r\rdata: c\n\n"]);
    assert_eq!(events, vec![ev("message", "a", None), ev("message", "b", None), ev("message", "c", None)]);
}

#[test]
fn split_anywhere_including_crlf_and_bom() {
    let whole: &[u8] = b"\xEF\xBB\xBFevent: frame\r\ndata: hello\r\ndata: world\r\n\r\ndata: x\n\n";
    let expected = vec![ev("frame", "hello\nworld", None), ev("message", "x", None)];
    for size in 1..whole.len() {
        let chunks: Vec<&[u8]> = whole.chunks(size).collect();
        assert_eq!(parse(&chunks), expected, "chunk size {size}");
    }
}

#[test]
fn an_unfinished_event_waits_for_its_blank_line() {
    let mut parser = SseParser::default();
    parser.push(Bytes::from_static(b"data: partial\n"));
    assert_eq!(parser.next_event(), None);
    parser.push(Bytes::from_static(b"\n"));
    assert_eq!(parser.next_event(), Some(ev("message", "partial", None)));
}
