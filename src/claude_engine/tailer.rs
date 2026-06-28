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

/// A subagent transcript untouched for this long is treated as no-longer-live (it
/// finished without a clean final message, or was orphaned). Generous enough to span a
/// quiet long-running command, tight enough that a dead subagent can't pin the parent
/// session busy — and thus block follow-ups — for more than this window.
const SUBAGENT_STALE: Duration = Duration::from_secs(180);

/// Whether a transcript file was modified within `SUBAGENT_STALE` (still being written).
fn transcript_is_fresh(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().map(|e| e < SUBAGENT_STALE).unwrap_or(true))
        .unwrap_or(false)
}

/// Poll a single session's transcript and stream translated events.
pub fn spawn_tailer(engine: Arc<ClaudeEngine>, session_id: String) {
    tokio::spawn(async move {
        // message id → last emitted content hash
        let mut emitted: HashMap<String, u64> = HashMap::new();
        // "<agentId>:<message id>" → last emitted content hash (subagent transcripts)
        let mut emitted_sub: HashMap<String, u64> = HashMap::new();
        let mut last_uuid: Option<String> = None;
        let mut last_len: u64 = 0;
        let mut idle_ticks: u32 = 0;
        // A subagent is still streaming (its transcript grows even when the main file
        // doesn't), so the cheap skip-on-unchanged path must stay disabled while pending.
        let mut has_pending_sub = false;
        // A background task's output file grows independently of the main transcript too,
        // so its tail must keep being re-read while the command runs.
        let mut has_pending_bg = false;

        loop {
            // Poll fast for near-realtime block delivery, but only re-parse when the
            // transcript actually grows (a cheap stat otherwise). claude persists
            // each content block as it completes, so this surfaces them within ~100ms.
            tokio::time::sleep(Duration::from_millis(100)).await;

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
                last_len = 0; // force a re-read of the new lineage file
            }

            let Some(path) = claude_cli::locate_jsonl(&uuid) else {
                continue;
            };

            // Skip the parse entirely when the file hasn't grown since last tick —
            // unless a subagent is still running (its own transcript may be growing).
            let cur_len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if cur_len == last_len && idle_ticks > 0 && !has_pending_sub && !has_pending_bg {
                idle_ticks = idle_ticks.saturating_add(1);
                if !entry.busy && idle_ticks > 600 {
                    if let Ok(mut t) = engine.tailers.lock() {
                        t.remove(&session_id);
                    }
                    break;
                }
                continue;
            }
            last_len = cur_len;

            let mut parsed = jsonl::parse_file(&path, &entry.id);
            // Fill task parts' running/completed state from the child transcripts.
            jsonl::enrich_subagents(&mut parsed);
            // Tail background-task output files into their parts.
            jsonl::enrich_background_tasks(&mut parsed);
            // Keep re-reading output files while any background command is still running.
            has_pending_bg = jsonl::has_running_background_task(&parsed);
            let ts = now_ms();

            if let Some(title) = &parsed.title {
                engine.set_title(&session_id, title, false);
            }

            // Per-subagent title + completion, read off the enriched `task` parts.
            let mut sub_meta: HashMap<String, (String, bool)> = HashMap::new();
            for m in &parsed.messages {
                for p in &m.parts {
                    if p.get("tool").and_then(|t| t.as_str()) != Some("task") {
                        continue;
                    }
                    let Some(aid) = p
                        .get("state")
                        .and_then(|s| s.get("metadata"))
                        .and_then(|m| m.get("sessionId"))
                        .and_then(|v| v.as_str())
                    else {
                        continue;
                    };
                    let title = p
                        .get("state")
                        .and_then(|s| s.get("title"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("Subagent")
                        .to_string();
                    let completed = p
                        .get("state")
                        .and_then(|s| s.get("status"))
                        .and_then(|s| s.as_str())
                        == Some("completed");
                    sub_meta.insert(aid.to_string(), (title, completed));
                }
            }

            // Stream each subagent's transcript as a child session (sessionID = agentId)
            // so the web UI renders it inline under the `task` tool AND nests it in the
            // sidebar, opencode-style.
            has_pending_sub = false;
            for aid in &parsed.subagent_ids {
                let (title, _) = sub_meta.get(aid).cloned().unwrap_or_default();
                engine.ensure_subagent_session(&session_id, aid, &title, &dir);
                let Some(sub_path) = claude_cli::locate_subagent_jsonl(aid) else {
                    // Launched but no transcript yet. This window is already covered by
                    // the main agent's `state=working`, so do NOT mark pending here — a
                    // subagent that never materializes must not wedge the session.
                    continue;
                };
                let sub = jsonl::parse_file(&sub_path, aid);
                for msg in &sub.messages {
                    let mid = msg
                        .info
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    if mid.is_empty() {
                        continue;
                    }
                    let key = format!("{aid}:{mid}");
                    let h = events::message_hash(msg);
                    if emitted_sub.get(&key) != Some(&h) {
                        events::emit_message(&engine, &dir, aid, msg, ts);
                        emitted_sub.insert(key, h);
                    }
                }
                // A subagent counts as in-flight ONLY if its transcript is unfinished AND
                // still being written. A stale transcript means it finished without a
                // clean final message or was orphaned — never let that wedge the parent.
                let (done, _) = jsonl::subagent_completed(&sub);
                let running = !done && transcript_is_fresh(&sub_path);
                engine.set_busy(aid, running);
                if running {
                    has_pending_sub = true;
                }
            }
            // Publish to the registry so the status poller keeps the session busy while a
            // subagent genuinely runs past the main agent's `state=done` — bounded by
            // liveness above so it can never pin the session busy forever.
            engine.set_subagent_pending(&session_id, has_pending_sub);

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
            // At 100ms/tick, 600 ticks ≈ 60s of quiet after idle.
            if any_new {
                idle_ticks = 0;
            } else if !entry.busy {
                idle_ticks += 1;
                if idle_ticks > 600 {
                    if let Ok(mut t) = engine.tailers.lock() {
                        t.remove(&session_id);
                    }
                    break;
                }
            } else {
                idle_ticks = 1; // busy: keep the skip-path counter primed but non-exiting
            }
        }
    });
}

/// How many consecutive polls a session's agent may be absent from `claude agents
/// --json --all` before we believe it's truly gone. A single transient glitch in the
/// background-service daemon must NOT flip a busy session to idle — that would flush a
/// queued follow-up into a `--resume`, spawning a competing main process that kills the
/// still-running (detached) subagents. ~3 × 2s ≈ 6s of confirmed absence.
const ABSENT_POLLS_BEFORE_IDLE: u32 = 3;

/// Poll `claude agents --json` and reconcile busy/idle for all known sessions.
pub fn spawn_status_poller(engine: Arc<ClaudeEngine>) {
    tokio::spawn(async move {
        // uuid → consecutive polls missing from the agent list (debounce, see above).
        let mut absent: HashMap<String, u32> = HashMap::new();
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

            // Snapshot sessions to avoid holding the lock across emits. Skip subagent
            // child rows — their busy/liveness is owned by the parent's tailer.
            let sessions: Vec<(String, Option<String>)> = engine
                .reg
                .lock()
                .map(|r| {
                    r.sessions
                        .values()
                        .filter(|s| !s.is_subagent)
                        .map(|s| (s.id.clone(), s.claude_session_id.clone()))
                        .collect()
                })
                .unwrap_or_default();

            for (id, uuid) in sessions {
                let Some(uuid) = uuid else { continue };
                // A turn being spawned isn't in `claude agents` yet; don't race it to
                // "idle" (which would flush the queue into a second, competing turn).
                if engine.is_dispatching(&id) {
                    continue;
                }

                let agent_busy = match busy_by_uuid.get(&uuid).copied() {
                    Some(b) => {
                        absent.remove(&uuid);
                        b
                    }
                    None => {
                        // Absent from the list. Treat as still-busy until it's been
                        // missing for several polls — a transient daemon hiccup must not
                        // trigger a subagent-killing resume.
                        let n = absent.entry(uuid.clone()).or_insert(0);
                        *n += 1;
                        if *n < ABSENT_POLLS_BEFORE_IDLE {
                            continue;
                        }
                        false
                    }
                };

                // The main agent flips to `state=done` while an async subagent is still
                // running; the transcript-derived `subagent_pending` keeps the session
                // busy until the subagent actually finishes (so we never resume into it).
                let busy = agent_busy || engine.subagent_pending(&id);

                // Recover live streaming after an opman restart: any still-running
                // (detached) agent needs a tailer re-spawned so its (and its subagents')
                // output streams again instead of appearing stalled/lost.
                if busy {
                    engine.clone().ensure_tailer(&id);
                }

                let went_idle = engine.set_busy(&id, busy);
                // On the busy → idle edge, send any follow-up queued while it ran. This is
                // the only place a `--resume` happens for a queued turn — never on a live
                // agent — so the agent's subagents are never orphaned mid-flight.
                if went_idle {
                    if let Some(text) = engine.take_pending(&id) {
                        engine.clone().spawn_turn(id.clone(), text);
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
