//! Translate parsed transcript messages into opencode SSE events.
//!
//! Part/message ids are content-stable (assistant `message.id`, tool `tool_use` id,
//! deterministic `msg_user_<sid>_<n>` for user turns), so re-emitting the whole
//! transcript is idempotent for the web UI (it upserts by id). The tailer therefore
//! re-parses on each change and emits only messages whose content hash changed.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde_json::json;

use super::jsonl::MsgOut;
use super::ClaudeEngine;

/// Stable hash of a message's rendered content (info + parts).
pub fn message_hash(msg: &MsgOut) -> u64 {
    let mut h = DefaultHasher::new();
    msg.info.to_string().hash(&mut h);
    for p in &msg.parts {
        p.to_string().hash(&mut h);
    }
    h.finish()
}

/// Emit `message.updated` + a `message.part.updated` per part, opencode-style.
pub fn emit_message(
    engine: &ClaudeEngine,
    directory: &str,
    session_id: &str,
    msg: &MsgOut,
    ts: u64,
) {
    engine.emit(directory, "message.updated", json!({ "info": msg.info }));
    for part in &msg.parts {
        engine.emit(
            directory,
            "message.part.updated",
            json!({ "sessionID": session_id, "part": part, "time": ts }),
        );
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod events_tests;
