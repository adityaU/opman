//! `session/update` → opencode SSE.
//!
//! Every variant ACP defines is handled here and nowhere else, so supporting a new agent
//! is never a rendering change: two agents that both speak ACP produce the same events.
//! Unknown `sessionUpdate` values are ignored rather than treated as errors — the protocol
//! is versioned and additive, and a newer agent must not break an older opman.

use std::sync::Arc;

use serde_json::{json, Value};

use super::attach::Prompt;
use super::emit::{Chunk, Emit};
use super::AcpEngine;

/// Apply one update to a session's transcript and broadcast what changed.
pub fn apply(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    let Some(kind) = update.get("sessionUpdate").and_then(Value::as_str) else {
        return;
    };
    match kind {
        "agent_message_chunk" => chunk(engine, session_id, update, Chunk::Text),
        "agent_thought_chunk" => chunk(engine, session_id, update, Chunk::Reasoning),
        "user_message_chunk" => user_chunk(engine, session_id, update),
        "tool_call" | "tool_call_update" => tool(engine, session_id, update),
        "plan" => plan(engine, session_id, update),
        "usage_update" => usage(engine, session_id, update),
        "session_info_update" => info(engine, session_id, update),
        "available_commands_update" => commands(engine, session_id, update),
        "current_mode_update" => mode(engine, session_id, update),
        "config_option_update" => engine.merge_config_options(session_id, update),
        _ => {}
    }
}

/// Text arriving from the agent. The turn is marked busy here rather than on the prompt
/// call: an agent that streams is working, whatever its bookkeeping says.
fn chunk(engine: &Arc<AcpEngine>, session_id: &str, update: &Value, kind: Chunk) {
    let Some(text) = update
        .get("content")
        .and_then(|c| c.get("text"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let message_id = update.get("messageId").and_then(Value::as_str);
    mark_working(engine, session_id);
    let emits = engine.with_transcript(session_id, |t| {
        let mut out = Vec::new();
        t.chunk(kind, message_id, text, &mut out);
        out
    });
    broadcast(engine, session_id, emits);
}

/// Mark the session busy — unless this is a `session/load` replay, where the same frames
/// arrive with no turn running to clear the flag again. Opening an old session must not
/// leave it spinning forever.
fn mark_working(engine: &Arc<AcpEngine>, session_id: &str) {
    if engine.is_replaying(session_id) {
        return;
    }
    engine.set_busy(session_id, true);
}

/// The agent echoing a user message. Agents send these while replaying a `session/load`,
/// which is what rebuilds history after a restart. During a live turn opman has already
/// rendered the prompt it sent, so echoes outside a replay would duplicate it.
fn user_chunk(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    let Some(text) = update
        .get("content")
        .and_then(|c| c.get("text"))
        .and_then(Value::as_str)
    else {
        return;
    };
    if !engine.is_replaying(session_id) {
        return;
    }
    let emits = engine.with_transcript(session_id, |t| {
        let mut out = Vec::new();
        t.user_message(&Prompt::text(text), &mut out);
        out
    });
    broadcast(engine, session_id, emits);
}

fn tool(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    mark_working(engine, session_id);
    let emits = engine.with_transcript(session_id, |t| {
        let mut out = Vec::new();
        t.tool_upsert(update, &mut out);
        out
    });
    broadcast(engine, session_id, emits);
    register_subagent(engine, session_id, update);
}

/// A `Task` tool call launches a subagent. ACP has no notion of child sessions, so opman
/// nests one only for agents whose transcripts it can actually read (see
/// `AgentConfig::subagent_transcripts`); for everyone else the call renders inline.
fn register_subagent(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    if !engine.agent.subagent_transcripts {
        return;
    }
    let is_task = update
        .get("_meta")
        .and_then(|m| m.get("claudeCode"))
        .and_then(|c| c.get("toolName"))
        .and_then(Value::as_str)
        .is_some_and(|n| n == "Task" || n == "Agent");
    if !is_task {
        return;
    }
    let Some(agent_id) = update
        .get("_meta")
        .and_then(|m| m.get("claudeCode"))
        .and_then(|c| c.get("agentId"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let dir = engine
        .get_session(session_id)
        .map(|s| s.directory)
        .unwrap_or_default();
    engine.ensure_subagent_session(session_id, agent_id, title, &dir);
}

/// The agent's task list. opman already has a todo panel driven by `todo.updated`, so a
/// plan maps onto it directly instead of being rendered as a pseudo tool call.
fn plan(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    let Some(entries) = update.get("entries").and_then(Value::as_array) else {
        return;
    };
    let todos: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            json!({
                "id": format!("{session_id}_plan_{i}"),
                "content": e.get("content").and_then(Value::as_str).unwrap_or(""),
                "status": plan_status(e.get("status").and_then(Value::as_str).unwrap_or("")),
                "priority": e.get("priority").and_then(Value::as_str).unwrap_or("medium"),
            })
        })
        .collect();
    engine.set_todos(session_id, todos.clone());
    let dir = engine
        .get_session(session_id)
        .map(|s| s.directory)
        .unwrap_or_default();
    engine.emit(
        &dir,
        "todo.updated",
        json!({ "sessionID": session_id, "todos": todos }),
    );
}

/// ACP plan statuses use the same vocabulary opman's todo panel does, apart from the
/// in-flight one.
fn plan_status(status: &str) -> &str {
    match status {
        "in_progress" => "in_progress",
        "completed" => "completed",
        _ => "pending",
    }
}

/// Context and cost for the turn. This is what the `claude -p` engine could never report:
/// it had no channel for usage, so every session showed zero tokens and zero cost.
fn usage(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    let used = update.get("used").and_then(Value::as_u64).unwrap_or(0);
    let size = update.get("size").and_then(Value::as_u64).unwrap_or(0);
    let cost = update
        .get("cost")
        .and_then(|c| c.get("amount"))
        .and_then(Value::as_f64);
    let tokens = json!({
        "input": used,
        "output": 0,
        "reasoning": 0,
        "context": { "used": used, "size": size },
        "cache": { "read": 0, "write": 0 },
    });
    let emits = engine.with_transcript(session_id, |t| {
        let mut out = Vec::new();
        t.set_usage(tokens, cost, &mut out);
        out
    });
    broadcast(engine, session_id, emits);
}

fn info(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    if let Some(title) = update.get("title").and_then(Value::as_str) {
        if !title.is_empty() {
            engine.set_title(session_id, title, false);
        }
    }
}

fn commands(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    let Some(list) = update.get("availableCommands").and_then(Value::as_array) else {
        return;
    };
    engine.set_commands(session_id, list.clone());
}

fn mode(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    if let Some(id) = update.get("currentModeId").and_then(Value::as_str) {
        engine.note_mode(session_id, id);
    }
}

/// Broadcast a transcript's emissions as opencode SSE events.
pub fn broadcast(engine: &Arc<AcpEngine>, session_id: &str, emits: Vec<Emit>) {
    if emits.is_empty() {
        return;
    }
    let Some(dir) = engine.get_session(session_id).map(|s| s.directory) else {
        return;
    };
    let ts = super::now_ms();
    for emit in emits {
        match emit {
            Emit::Message(info) => engine.emit(&dir, "message.updated", json!({ "info": info })),
            Emit::Part(part) => engine.emit(
                &dir,
                "message.part.updated",
                json!({ "sessionID": session_id, "time": ts, "part": part }),
            ),
            Emit::Delta {
                session_id,
                message_id,
                part_id,
                delta,
            } => engine.emit(
                &dir,
                "message.part.delta",
                json!({
                    "sessionID": session_id,
                    "messageID": message_id,
                    "partID": part_id,
                    "field": "text",
                    "delta": delta,
                }),
            ),
        }
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
