//! Coverage for the partial-message accumulator: deltas accumulate into growing text,
//! part ids line up with the transcript parser's, non-text deltas are ignored, and
//! frames arriving out of order never panic.

use super::*;
use crate::claude_p_engine::ClaudePEngine;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

fn drain(
    rx: &mut tokio::sync::broadcast::Receiver<crate::claude_engine::EngineEvent>,
) -> Vec<Value> {
    let mut out = vec![];
    while let Ok(ev) = rx.try_recv() {
        out.push(serde_json::from_str(&ev.data).unwrap());
    }
    out
}

/// The text of every emitted `message.part.updated`, in order.
fn part_texts(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["type"] == "message.part.updated")
        .map(|e| {
            e["properties"]["part"]["text"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

fn start_frame() -> Value {
    json!({
        "type": "message_start",
        "message": {
            "id": "msg_abc", "model": "claude-opus-5", "role": "assistant",
            "usage": { "input_tokens": 10, "output_tokens": 7, "cache_read_input_tokens": 3 }
        }
    })
}

fn block_start(index: u64, kind: &str) -> Value {
    json!({ "type": "content_block_start", "index": index, "content_block": { "type": kind } })
}

fn text_delta(index: u64, text: &str) -> Value {
    json!({ "type": "content_block_delta", "index": index,
            "delta": { "type": "text_delta", "text": text } })
}

#[test]
fn deltas_accumulate_into_growing_text() {
    let e = engine();
    let mut rx = e.subscribe();
    let mut p = Partial::default();

    p.handle(&e, "s1", "d", &start_frame());
    p.handle(&e, "s1", "d", &block_start(0, "text"));
    p.handle(&e, "s1", "d", &text_delta(0, "Hel"));
    p.handle(&e, "s1", "d", &text_delta(0, "lo w"));
    p.handle(&e, "s1", "d", &text_delta(0, "orld"));

    let events = drain(&mut rx);
    assert_eq!(part_texts(&events), vec!["Hel", "Hello w", "Hello world"]);
}

#[test]
fn message_start_emits_assistant_info_with_tokens() {
    let e = engine();
    let mut rx = e.subscribe();
    let mut p = Partial::default();

    p.handle(&e, "s1", "d", &start_frame());

    let events = drain(&mut rx);
    let info = &events[0]["properties"]["info"];
    assert_eq!(events[0]["type"], "message.updated");
    assert_eq!(info["role"], "assistant");
    assert_eq!(info["id"], "msg_abc");
    assert_eq!(info["sessionID"], "s1");
    assert_eq!(info["model"], "claude-opus-5");
    assert_eq!(info["tokens"]["input"], 10);
    assert_eq!(info["tokens"]["cache"]["read"], 3);
}

#[test]
fn part_ids_match_the_transcript_parser_shape() {
    let e = engine();
    let mut rx = e.subscribe();
    let mut p = Partial::default();

    p.handle(&e, "s1", "d", &start_frame());
    p.handle(&e, "s1", "d", &block_start(0, "thinking"));
    p.handle(
        &e,
        "s1",
        "d",
        &json!({ "type": "content_block_delta", "index": 0,
        "delta": { "type": "thinking_delta", "thinking": "hmm" } }),
    );
    p.handle(&e, "s1", "d", &block_start(1, "text"));
    p.handle(&e, "s1", "d", &text_delta(1, "answer"));

    let events = drain(&mut rx);
    let parts: Vec<_> = events
        .iter()
        .filter(|x| x["type"] == "message.part.updated")
        .collect();
    assert_eq!(parts[0]["properties"]["part"]["id"], "msg_abc:0");
    assert_eq!(parts[0]["properties"]["part"]["type"], "reasoning");
    assert_eq!(parts[0]["properties"]["part"]["messageID"], "msg_abc");
    assert_eq!(parts[1]["properties"]["part"]["id"], "msg_abc:1");
    assert_eq!(parts[1]["properties"]["part"]["type"], "text");
}

#[test]
fn non_text_deltas_and_unstreamable_blocks_emit_nothing() {
    let e = engine();
    let mut rx = e.subscribe();
    let mut p = Partial::default();

    p.handle(&e, "s1", "d", &start_frame());
    let _ = drain(&mut rx);

    // signature_delta on a streamed block carries no visible text.
    p.handle(&e, "s1", "d", &block_start(0, "thinking"));
    p.handle(
        &e,
        "s1",
        "d",
        &json!({ "type": "content_block_delta", "index": 0,
        "delta": { "type": "signature_delta", "signature": "sig" } }),
    );
    // tool_use blocks are rendered by the authoritative re-parse, not streamed.
    p.handle(&e, "s1", "d", &block_start(1, "tool_use"));
    p.handle(
        &e,
        "s1",
        "d",
        &json!({ "type": "content_block_delta", "index": 1,
        "delta": { "type": "input_json_delta", "partial_json": "{\"a\":" } }),
    );

    assert!(part_texts(&drain(&mut rx)).is_empty());
}

#[test]
fn message_delta_refreshes_tokens() {
    let e = engine();
    let mut rx = e.subscribe();
    let mut p = Partial::default();

    p.handle(&e, "s1", "d", &start_frame());
    let _ = drain(&mut rx);
    p.handle(
        &e,
        "s1",
        "d",
        &json!({ "type": "message_delta", "usage": {
            "input_tokens": 10, "output_tokens": 53,
            "output_tokens_details": { "thinking_tokens": 43 } } }),
    );

    let events = drain(&mut rx);
    assert_eq!(events[0]["type"], "message.updated");
    assert_eq!(events[0]["properties"]["info"]["tokens"]["output"], 53);
    assert_eq!(events[0]["properties"]["info"]["tokens"]["reasoning"], 43);
}

#[test]
fn frames_without_a_message_start_are_ignored() {
    let e = engine();
    let mut rx = e.subscribe();
    let mut p = Partial::default();

    // Deltas before any message_start, and a delta for an index never opened.
    p.handle(&e, "s1", "d", &text_delta(0, "orphan"));
    p.handle(
        &e,
        "s1",
        "d",
        &json!({ "type": "message_delta", "usage": {} }),
    );
    p.handle(&e, "s1", "d", &start_frame());
    p.handle(&e, "s1", "d", &text_delta(9, "gap"));
    p.handle(&e, "s1", "d", &json!({ "type": "message_start" }));

    assert!(part_texts(&drain(&mut rx)).is_empty());
}
