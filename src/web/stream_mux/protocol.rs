//! Wire frames of the stream socket. See the module docs in `mod.rs` for the protocol.

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use super::sse_parse::SseEvent;

/// What the client may ask for.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "lowercase")]
pub(super) enum ClientFrame {
    Sub { id: u32, path: String },
    Unsub { id: u32 },
}

#[derive(Serialize)]
struct EventFrame<'a> {
    id: u32,
    event: &'a str,
    data: &'a str,
    #[serde(rename = "lastId", skip_serializing_if = "Option::is_none")]
    last_id: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Op {
    Open,
    End,
    Ping,
}

#[derive(Serialize)]
struct ControlFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    op: Op,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
}

fn encode(frame: &impl Serialize) -> String {
    // Plain structs of strings and integers always serialize.
    serde_json::to_string(frame).unwrap_or_default()
}

pub(super) fn event(id: u32, event: &SseEvent) -> String {
    encode(&EventFrame {
        id,
        event: &event.event,
        data: &event.data,
        last_id: event.last_id.as_deref(),
    })
}

pub(super) fn open(id: u32) -> String {
    encode(&ControlFrame {
        id: Some(id),
        op: Op::Open,
        status: None,
    })
}

/// `refused` carries the HTTP status of a stream that never opened.
pub(super) fn end(id: u32, refused: Option<StatusCode>) -> String {
    encode(&ControlFrame {
        id: Some(id),
        op: Op::End,
        status: refused.map(|s| s.as_u16()),
    })
}

pub(super) fn ping() -> String {
    encode(&ControlFrame {
        id: None,
        op: Op::Ping,
        status: None,
    })
}
