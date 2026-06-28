//! Live transcript tailer + busy/idle status poller for the Claude engine.
//!
//! - `spawn_tailer`: per opman session, polls the *latest* claude transcript JSONL
//!   (resume writes a fresh file containing full history, so the latest file is the
//!   whole conversation). On change it re-parses and emits opencode events for any
//!   message whose content changed, plus the `ai-title` → `session.updated`.
//! - `spawn_status_poller`: polls `claude agents --json` and reconciles each
//!   session's busy/idle state (emitting `session.status`/`session.idle`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::{claude_cli, events, jsonl, now_ms, ClaudeEngine};

/// Poll a single session's transcript and stream translated events.
pub fn spawn_tailer(engine: Arc<ClaudeEngine>, session_id: String) {
    tokio::spawn(async move {
        // message id → last emitted content hash
        let mut emitted: HashMap<String, u64> = HashMap::new();
        let mut last_uuid: Option<String> = None;
        let mut idle_ticks: u32 = 0;

        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;

            let Some(entry) = engine.get_session(&session_id) else {
                break; // session gone
            };
            let dir = entry.directory.clone();
            let Some(uuid) = entry.claude_session_id.clone() else {
                continue; // no background turn yet
            };

            // On a new lineage turn, the fresh file repeats history under the same
            // stable ids, so existing hashes still match — only new turns re-emit.
            if last_uuid.as_deref() != Some(uuid.as_str()) {
                last_uuid = Some(uuid.clone());
            }

            let Some(path) = claude_cli::locate_jsonl(&uuid) else {
                continue;
            };

            let parsed = jsonl::parse_file(&path, &entry.id);
            let ts = now_ms();

            if let Some(title) = &parsed.title {
                engine.set_title(&session_id, title);
            }

            let mut any_new = false;
            for msg in &parsed.messages {
                let id = msg
                    .info
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                let h = events::message_hash(msg);
                if emitted.get(&id) != Some(&h) {
                    events::emit_message(&engine, &dir, &entry.id, msg, ts);
                    emitted.insert(id, h);
                    any_new = true;
                }
            }

            // Stop tailing a session that has been idle and unchanged for a while
            // (the status poller still tracks it; reopening a turn re-spawns a tailer).
            if any_new {
                idle_ticks = 0;
            } else if !entry.busy {
                idle_ticks += 1;
                if idle_ticks > 150 {
                    // ~60s of quiet after idle
                    if let Ok(mut t) = engine.tailers.lock() {
                        t.remove(&session_id);
                    }
                    break;
                }
            }
        }
    });
}

/// Poll `claude agents --json` and reconcile busy/idle for all known sessions.
pub fn spawn_status_poller(engine: Arc<ClaudeEngine>) {
    tokio::spawn(async move {
        loop {
            // Run the (blocking) CLI call off the async reactor.
            let agents = tokio::task::spawn_blocking(|| claude_cli::agents_json(None))
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();

            // uuid → busy
            let mut busy_by_uuid: HashMap<String, bool> = HashMap::new();
            for a in &agents {
                if !a.session_id.is_empty() {
                    busy_by_uuid.insert(a.session_id.clone(), a.is_busy());
                }
            }

            // Snapshot sessions to avoid holding the lock across emits.
            let sessions: Vec<(String, Option<String>)> = engine
                .reg
                .lock()
                .map(|r| {
                    r.sessions
                        .values()
                        .map(|s| (s.id.clone(), s.claude_session_id.clone()))
                        .collect()
                })
                .unwrap_or_default();

            for (id, uuid) in sessions {
                if let Some(uuid) = uuid {
                    if let Some(&busy) = busy_by_uuid.get(&uuid) {
                        engine.set_busy(&id, busy);
                    } else {
                        // Not in the agent list (completed & reaped) → idle.
                        engine.set_busy(&id, false);
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
