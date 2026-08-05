//! Token-level streaming for `claude -p`.
//!
//! With `--include-partial-messages` the CLI emits Anthropic `stream_event` frames
//! (`message_start`, `content_block_start` / `_delta` / `_stop`, `message_delta`)
//! interleaved with the coarse per-block `assistant` frames. Without them a reply only
//! becomes visible once a whole content block lands on disk — the "all at once" feel.
//! This module folds those deltas into growing text and emits an opencode part update
//! per chunk.
//!
//! Part ids mirror the shared transcript parser's `{message_id}:{block_index}`, so the
//! authoritative re-parse that follows each completed block overwrites the streamed
//! part in place rather than appending a duplicate.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{now_ms, ClaudePEngine};
use crate::claude_engine::jsonl;

/// The assistant message currently being streamed, and its per-block text so far.
#[derive(Default)]
pub(super) struct Partial {
    id: String,
    model: String,
    created: u64,
    tokens: Value,
    /// `(opencode part type, accumulated text)` indexed by content-block index. An
    /// empty part type marks a block we don't stream (tool_use — the re-parse renders
    /// those once their input JSON is complete).
    blocks: Vec<(&'static str, String)>,
}

/// opencode part type for an Anthropic content-block type, or `None` if not streamable.
fn part_type(block_type: &str) -> Option<&'static str> {
    match block_type {
        "text" => Some("text"),
        "thinking" => Some("reasoning"),
        _ => None,
    }
}

/// Text carried by a `content_block_delta`, or `None` for deltas that add no visible
/// text (`signature_delta`, `input_json_delta`).
fn delta_text(delta: &Value) -> Option<&str> {
    match delta.get("type").and_then(|t| t.as_str())? {
        "text_delta" => delta.get("text")?.as_str(),
        "thinking_delta" => delta.get("thinking")?.as_str(),
        _ => None,
    }
}

impl Partial {
    /// Fold one `stream_event` payload into the in-flight message, emitting the
    /// incremental opencode events it produces.
    pub(super) fn handle(
        &mut self,
        engine: &Arc<ClaudePEngine>,
        session_id: &str,
        directory: &str,
        event: &Value,
    ) {
        match event.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "message_start" => self.start(engine, session_id, directory, event),
            "content_block_start" => self.block_start(event),
            "content_block_delta" => self.block_delta(engine, session_id, directory, event),
            "message_delta" => self.usage_update(engine, session_id, directory, event),
            _ => {}
        }
    }

    fn start(
        &mut self,
        engine: &Arc<ClaudePEngine>,
        session_id: &str,
        directory: &str,
        event: &Value,
    ) {
        let Some(msg) = event.get("message") else {
            return;
        };
        let Some(id) = msg.get("id").and_then(|i| i.as_str()) else {
            return;
        };
        self.id = id.to_string();
        self.model = msg
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or_default()
            .to_string();
        self.created = now_ms();
        self.tokens = msg
            .get("usage")
            .map(jsonl::tokens_from_usage)
            .unwrap_or(json!({}));
        self.blocks.clear();
        self.emit_info(engine, session_id, directory);
    }

    fn block_start(&mut self, event: &Value) {
        let Some(idx) = event.get("index").and_then(|i| i.as_u64()) else {
            return;
        };
        let kind = event
            .get("content_block")
            .and_then(|b| b.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let idx = idx as usize;
        if self.blocks.len() <= idx {
            self.blocks.resize_with(idx + 1, || ("", String::new()));
        }
        self.blocks[idx] = (part_type(kind).unwrap_or(""), String::new());
    }

    fn block_delta(
        &mut self,
        engine: &Arc<ClaudePEngine>,
        session_id: &str,
        directory: &str,
        event: &Value,
    ) {
        if self.id.is_empty() {
            return;
        }
        let Some(idx) = event.get("index").and_then(|i| i.as_u64()) else {
            return;
        };
        let Some(text) = event.get("delta").and_then(delta_text) else {
            return;
        };
        let Some(block) = self.blocks.get_mut(idx as usize) else {
            return;
        };
        if block.0.is_empty() {
            return;
        }
        block.1.push_str(text);

        let (ptype, acc) = (block.0, block.1.as_str());
        engine.emit(
            directory,
            "message.part.updated",
            json!({ "sessionID": session_id, "time": now_ms(), "part": {
                "type": ptype,
                "id": format!("{}:{}", self.id, idx),
                "messageID": self.id,
                "sessionID": session_id,
                "text": acc,
            }}),
        );
    }

    fn usage_update(
        &mut self,
        engine: &Arc<ClaudePEngine>,
        session_id: &str,
        directory: &str,
        event: &Value,
    ) {
        if self.id.is_empty() {
            return;
        }
        let Some(usage) = event.get("usage") else {
            return;
        };
        self.tokens = jsonl::tokens_from_usage(usage);
        self.emit_info(engine, session_id, directory);
    }

    /// Emit the assistant message envelope in the same shape the transcript parser
    /// produces, so a streamed message and its re-parsed form are the same message.
    fn emit_info(&self, engine: &Arc<ClaudePEngine>, session_id: &str, directory: &str) {
        engine.emit(
            directory,
            "message.updated",
            json!({ "info": {
                "role": "assistant",
                "id": self.id,
                "sessionID": session_id,
                "model": self.model,
                "cost": 0.0,
                "tokens": self.tokens,
                "time": { "created": self.created },
            }}),
        );
    }
}

#[cfg(test)]
#[path = "stream_delta_tests.rs"]
mod stream_delta_tests;
