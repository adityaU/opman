//! Opencode event handler — dispatches parsed SSE events to SseState signals.
//!
//! Session/permission/question/file-edit handlers live in `session_handlers`.

use leptos::prelude::Set;

use crate::hooks::use_sse_state::SseState;
use crate::sse::message_map::{self, MessageMap};
use crate::types::api::SessionStats;

use super::session_handlers;

/// Handle a parsed opencode event, dispatching to SseState signals.
pub fn handle_opencode_event(sse: &SseState, event: &serde_json::Value) {
    let event_type = event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let props = event
        .get("properties")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    match event_type {
        // ── Message map events ─────────────────────────────────────
        "message.updated" => handle_message_updated(sse, &props),
        "message.part.updated" => handle_message_part_updated(sse, &props),
        "message.part.delta" => handle_message_part_delta(sse, &props),
        "message.removed" => handle_message_removed(sse, &props),
        "message.part.removed" => handle_message_part_removed(sse, &props),

        // ── Session status events ──────────────────────────────────
        "session.status" => session_handlers::handle_session_status(sse, &props),

        "session.created" | "session.deleted" => {
            // No-op: the server will also emit a `state_changed` SSE event,
            // which the `state_changed` handler already handles.
        }

        "session.updated" => session_handlers::handle_session_updated(sse, &props),

        // ── Permission events ──────────────────────────────────────
        "permission.asked" => session_handlers::handle_permission_asked(sse, &props),
        "permission.replied" => session_handlers::handle_permission_replied(sse, &props),

        // ── Question events ────────────────────────────────────────
        "question.asked" => session_handlers::handle_question_asked(sse, &props),
        "question.replied" | "question.rejected" => {
            session_handlers::handle_question_replied(sse, &props)
        }

        // ── File edits ─────────────────────────────────────────────
        "file.edited" => session_handlers::handle_file_edited(sse, &props),

        // ── Todo events ────────────────────────────────────────────
        "todo.updated" => {
            if let Some(window) = web_sys::window() {
                let _ = window
                    .dispatch_event(&web_sys::CustomEvent::new("opman:todo-updated").unwrap());
            }
        }

        _ => {} // Unhandled events silently ignored
    }
}

fn is_other_session(sse: &SseState, session_id: &str) -> bool {
    let active = sse.tracked_session_id();
    active.is_some() && !session_id.is_empty() && active.as_deref() != Some(session_id)
}

fn handle_message_updated(sse: &SseState, props: &serde_json::Value) {
    let info = match props.get("info") {
        Some(i) => i,
        None => return,
    };
    let msg_session_id = info
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if is_other_session(sse, msg_session_id) {
        // Route to subagent map instead of dropping
        if !msg_session_id.is_empty() {
            let sid = msg_session_id.to_string();
            let info_clone = info.clone();
            sse.update_subagent_map(&sid, move |map| {
                message_map::upsert_message_info(map, &info_clone)
            });
        }
        return;
    }

    // When the server echoes a real user message, remove any optimistic entry
    let is_user = info
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        == "user";

    sse.update_message_map(|map: &mut MessageMap| {
        let changed = message_map::upsert_message_info(map, info);
        if is_user {
            map.retain(|k, _| !k.starts_with("__optimistic__"));
        }
        changed
    });

    // Update stats from message info
    if info.get("cost").is_some() || info.get("tokens").is_some() {
        let cost = info.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let tokens = info.get("tokens");
        let input = tokens
            .and_then(|t| t.get("input"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output = tokens
            .and_then(|t| t.get("output"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let reasoning = tokens
            .and_then(|t| t.get("reasoning"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = tokens
            .and_then(|t| t.get("cache"))
            .and_then(|c| c.get("read"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_write = tokens
            .and_then(|t| t.get("cache"))
            .and_then(|c| c.get("write"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        sse.set_stats.set(Some(SessionStats {
            session_id: None,
            cost,
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: reasoning,
            cache_read,
            cache_write,
        }));
    }
}

fn handle_message_part_updated(sse: &SseState, props: &serde_json::Value) {
    let part = match props.get("part") {
        Some(p) => p,
        None => return,
    };
    let part_session_id = part
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if is_other_session(sse, part_session_id) {
        if !part_session_id.is_empty() {
            let sid = part_session_id.to_string();
            let part_clone = part.clone();
            sse.update_subagent_map(&sid, move |map| message_map::upsert_part(map, &part_clone));
        }
        return;
    }

    sse.update_message_map(|map: &mut MessageMap| message_map::upsert_part(map, part));
}

fn handle_message_part_delta(sse: &SseState, props: &serde_json::Value) {
    let session_id = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let message_id = props
        .get("messageID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let part_id = props
        .get("partID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let field = props
        .get("field")
        .and_then(|v| v.as_str())
        .unwrap_or("text");
    let delta = props
        .get("delta")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if message_id.is_empty() || part_id.is_empty() || delta.is_empty() {
        return;
    }

    if is_other_session(sse, session_id) {
        if !session_id.is_empty() {
            let sid_key = session_id.to_string();
            let sid = session_id.to_string();
            let mid = message_id.to_string();
            let pid = part_id.to_string();
            let fld = field.to_string();
            let dlt = delta.to_string();
            sse.update_subagent_map(&sid_key, move |map| {
                message_map::apply_part_delta(map, &sid, &mid, &pid, &fld, &dlt)
            });
        }
        return;
    }

    let sid = session_id.to_string();
    let mid = message_id.to_string();
    let pid = part_id.to_string();
    let fld = field.to_string();
    let dlt = delta.to_string();
    sse.update_message_map(move |map: &mut MessageMap| {
        message_map::apply_part_delta(map, &sid, &mid, &pid, &fld, &dlt)
    });
}

fn handle_message_removed(sse: &SseState, props: &serde_json::Value) {
    let msg_id = props
        .get("messageID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let rm_session_id = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if msg_id.is_empty() {
        return;
    }
    if is_other_session(sse, rm_session_id) {
        if !rm_session_id.is_empty() {
            let sid = rm_session_id.to_string();
            let mid = msg_id.to_string();
            sse.update_subagent_map(&sid, move |map| message_map::remove_message(map, &mid));
        }
        return;
    }

    let mid = msg_id.to_string();
    sse.update_message_map(move |map: &mut MessageMap| message_map::remove_message(map, &mid));
}

fn handle_message_part_removed(sse: &SseState, props: &serde_json::Value) {
    let msg_id = props
        .get("messageID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let part_id = props
        .get("partID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let rp_session_id = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if msg_id.is_empty() || part_id.is_empty() {
        return;
    }
    if is_other_session(sse, rp_session_id) {
        if !rp_session_id.is_empty() {
            let sid = rp_session_id.to_string();
            let mid = msg_id.to_string();
            let pid = part_id.to_string();
            sse.update_subagent_map(&sid, move |map| message_map::remove_part(map, &mid, &pid));
        }
        return;
    }

    let mid = msg_id.to_string();
    let pid = part_id.to_string();
    sse.update_message_map(move |map: &mut MessageMap| message_map::remove_part(map, &mid, &pid));
}
