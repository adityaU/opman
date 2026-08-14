//! The three operations that are about an agent rather than a turn: stop it, find it,
//! wait for it.
//!
//! They exist because the four original tools only compose in one direction. An agent
//! could start a peer and steer it, but it could not name a peer it had not started, could
//! not stop one that had wedged, and had no way to learn a reply had arrived short of
//! polling `agent_progress` and re-parsing a raw transcript on every tick. That last gap
//! is the one that mattered: a request/response exchange between two agents was not
//! expressible, so the manager was a fan-out and never a conversation.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use super::request::Page;
use super::ManagerState;
use crate::runner::SessionScope;

/// How often a wait re-reads the target's state.
const POLL: Duration = Duration::from_millis(400);

/// How long a wait will watch an idle target before deciding the turn it was waiting for
/// is never going to start. A dispatch is asynchronous, so a wait issued immediately after
/// a send routinely arrives before the target has picked the message up; returning "idle,
/// done" at that moment would report the *previous* turn's reply as this one's.
const SETTLE: Duration = Duration::from_secs(10);

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_TIMEOUT: Duration = Duration::from_secs(1800);

/// Stop whatever the target is doing.
pub(super) async fn abort(state: &ManagerState, target: &str, directory: &str) -> Result<Value> {
    state.registry.abort(target, directory).await?;
    Ok(json!({ "agent_id": target, "aborted": true }))
}

/// One page of the project's sessions, newest first, with the runner that owns each
/// and whether it is busy.
///
/// Paged rather than whole: a long-lived project runs to hundreds of sessions, and an
/// agent that only wants to address a peer it did not start pays for all of them in
/// context on every call. `count` is the project total, so a caller that needs an older
/// agent knows to ask for the next page.
pub(super) async fn list(state: &ManagerState, directory: &str, page: Page) -> Result<Value> {
    let mut sessions = state
        .registry
        .sessions_in(directory, SessionScope::All)
        .await?;
    let count = sessions.len();
    sessions.sort_unstable_by(|(_, a), (_, b)| b.time.updated.cmp(&a.time.updated));
    let running = running_ids(state, directory).await;
    let queues = state.queues.lock().await;
    let agents: Vec<Value> = sessions
        .iter()
        .skip(page.offset)
        .take(page.limit)
        .map(|(kind, session)| {
            json!({
                "agent_id": session.id,
                "title": session.title,
                "runner": kind,
                "busy": running.contains(&session.id),
                "queued_messages": queues.get(&session.id).map(Vec::len).unwrap_or(0),
            })
        })
        .collect();
    Ok(json!({
        "agents": agents,
        "returned": agents.len(),
        "offset": page.offset,
        "count": count,
    }))
}

/// Block until the target finishes its turn, then hand back what it said.
///
/// Reports `timed_out` rather than failing: a turn that is merely slow is not an error,
/// and the caller still wants the transcript it has so far.
pub(super) async fn wait(
    state: &ManagerState,
    target: &str,
    directory: &str,
    timeout: Option<u64>,
) -> Result<Value> {
    let timeout = timeout
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_TIMEOUT)
        .min(MAX_TIMEOUT);
    let deadline = tokio::time::Instant::now() + timeout;
    let settle = tokio::time::Instant::now() + SETTLE;
    let mut seen_busy = false;
    let mut progress = state.registry.progress(target, directory).await?;
    loop {
        let busy = progress.get("busy").and_then(Value::as_bool) == Some(true);
        seen_busy |= busy;
        let settled = seen_busy || tokio::time::Instant::now() >= settle;
        if !busy && settled {
            return Ok(finished(target, &progress, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(finished(target, &progress, true));
        }
        tokio::time::sleep(POLL).await;
        progress = state.registry.progress(target, directory).await?;
    }
}

fn finished(target: &str, progress: &Value, timed_out: bool) -> Value {
    json!({
        "agent_id": target,
        "timed_out": timed_out,
        "busy": progress.get("busy").and_then(Value::as_bool).unwrap_or(false),
        "reply": last_assistant_text(progress),
        "runner": progress.get("runner").cloned().unwrap_or(Value::Null),
    })
}

/// The most recent assistant turn as plain text.
///
/// `progress` carries the transcript newest-first, and every runner renders an assistant
/// message the same way once it reaches here: `info.role` plus text parts. Reasoning parts
/// are skipped — they are the model thinking, not the answer it gave.
fn last_assistant_text(progress: &Value) -> String {
    let Some(messages) = progress.get("messages").and_then(Value::as_array) else {
        return String::new();
    };
    for message in messages {
        if message["info"]["role"] != "assistant" {
            continue;
        }
        let Some(parts) = message.get("parts").and_then(Value::as_array) else {
            continue;
        };
        let text = parts
            .iter()
            .filter(|part| part["type"] == "text")
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// The union of the running sets across every runner.
///
/// A runner that cannot be reached contributes nothing rather than clearing the set: its
/// sessions are then reported idle, which is the same answer the caller would get from
/// `agent_progress`, and is better than claiming a session is running on no evidence.
async fn running_ids(state: &ManagerState, directory: &str) -> HashSet<String> {
    state
        .registry
        .status_all(directory)
        .await
        .into_iter()
        .filter_map(|(_, ids)| ids)
        .flatten()
        .collect()
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod ops_tests;
