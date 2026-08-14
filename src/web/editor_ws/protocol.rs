//! The editor channel's wire types.
//!
//! One MessagePack frame per message, multiplexed by request id. The shape is
//! deliberately narrow: an op name, an opaque payload, and an id — so adding an
//! operation is a new `Op` variant and a dispatch arm, not a new endpoint, a new
//! route, and a new client function.
//!
//! Why not JSON over the same socket: the payloads that move most are file
//! contents and completion lists, and MessagePack carries a string without
//! escaping it and a byte array without base64.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Everything the editor can ask for over the channel.
///
/// An enum rather than a free string so an unknown op is a decode error at the
/// edge, and so the dispatcher's match is exhaustive — a new operation cannot
/// be added without answering it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    // ── Language server ──
    Hover,
    Goto,
    References,
    Rename,
    Format,
    Completion,
    Diagnostics,
    // ── Files ──
    Browse,
    Read,
    Write,
    CreateFile,
    CreateDir,
    Delete,
    Move,
    /// Withdraw an earlier request. The only op that names another one.
    Cancel,
}

impl Op {
    /// Whether the op only reads. Used to decide what a cancel may abandon:
    /// a write that has already started must finish, or the file is left torn.
    pub fn is_read_only(self) -> bool {
        !matches!(
            self,
            Self::Write | Self::CreateFile | Self::CreateDir | Self::Delete | Self::Move | Self::Rename | Self::Format
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct Request {
    /// Monotonic per connection. Echoed on the response and named by a cancel.
    pub id: u64,
    pub op: Op,
    /// The op's arguments. Shapes match the REST bodies they replace, so the
    /// two front doors cannot drift apart.
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub id: u64,
    /// Present on success. `null` is a legitimate result for several ops, so
    /// success is signalled by the absence of `error`, not by this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Self { id, result: Some(result), error: None }
    }

    pub fn failed(id: u64, error: impl Into<String>) -> Self {
        Self { id, result: None, error: Some(error.into()) }
    }
}

/// Something the server says without being asked — the reason the channel
/// exists rather than a second REST client. Diagnostics arrive when the language
/// server publishes them instead of when a poll happens to land.
#[derive(Debug, Serialize)]
pub struct Event {
    /// Zero marks a frame as unsolicited; no request ever uses it.
    pub id: u64,
    pub event: &'static str,
    pub payload: Value,
}

impl Event {
    pub fn new(event: &'static str, payload: Value) -> Self {
        Self { id: 0, event, payload }
    }
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    // Named fields: the client decodes by key, so a field added on either side
    // does not shift the meaning of the ones already there.
    rmp_serde::to_vec_named(value)
}

pub fn decode(bytes: &[u8]) -> Result<Request, rmp_serde::decode::Error> {
    rmp_serde::from_slice(bytes)
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod protocol_tests;
