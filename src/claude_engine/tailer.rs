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

/// Mutable state carried across `tick_tailer` iterations (was the set of `let mut`
/// locals inside `spawn_tailer`'s loop). Extracted so a single iteration can be driven
/// directly in tests without spawning the polling task.
#[derive(Default)]
pub(crate) struct TailerState {
    /// message id → last emitted content hash
    emitted: HashMap<String, u64>,
    /// "<agentId>:<message id>" → last emitted content hash (subagent transcripts)
    emitted_sub: HashMap<String, u64>,
    last_uuid: Option<String>,
    last_len: u64,
    idle_ticks: u32,
    /// A subagent is still streaming (its transcript grows even when the main file
    /// doesn't), so the cheap skip-on-unchanged path must stay disabled while pending.
    has_pending_sub: bool,
    /// A background task's output file grows independently of the main transcript too,
    /// so its tail must keep being re-read while the command runs.
    has_pending_bg: bool,
}

/// Poll a single session's transcript and stream translated events.
pub fn spawn_tailer(engine: Arc<ClaudeEngine>, session_id: String) {
    tokio::spawn(async move {
        let mut st = TailerState::default();
        loop {
            // Poll fast for near-realtime block delivery, but only re-parse when the
            // transcript actually grows (a cheap stat otherwise). claude persists
            // each content block as it completes, so this surfaces them within ~100ms.
            tokio::time::sleep(Duration::from_millis(100)).await;
            if !tick_tailer(&engine, &session_id, &mut st) {
                break;
            }
        }
    });
}

/// Run one iteration of the tailer loop (everything after the poll sleep). Returns
/// `false` when the caller should break the loop (session gone, or idle-expired and
/// unregistered), `true` to keep polling. Behavior-preserving extraction of
/// `spawn_tailer`'s loop body so it can be unit-tested against a temp transcript dir.
pub(crate) fn tick_tailer(
    engine: &Arc<ClaudeEngine>,
    session_id: &str,
    st: &mut TailerState,
) -> bool {
    let Some(entry) = engine.get_session(session_id) else {
        return false; // session gone
    };
    let dir = entry.directory.clone();
    let Some(uuid) = entry.claude_session_id.clone() else {
        return true; // no background turn yet
    };

    // On a new lineage turn, the fresh file repeats history under the same
    // stable ids, so existing hashes still match — only new turns re-emit.
    if st.last_uuid.as_deref() != Some(uuid.as_str()) {
        st.last_uuid = Some(uuid.clone());
        st.last_len = 0; // force a re-read of the new lineage file
    }

    let Some(path) = claude_cli::locate_jsonl(&uuid) else {
        return true;
    };

    // Skip the parse entirely when the file hasn't grown since last tick —
    // unless a subagent is still running (its own transcript may be growing).
    let cur_len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if cur_len == st.last_len && st.idle_ticks > 0 && !st.has_pending_sub && !st.has_pending_bg {
        st.idle_ticks = st.idle_ticks.saturating_add(1);
        if !entry.busy && st.idle_ticks > 600 {
            if let Ok(mut t) = engine.tailers.lock() {
                t.remove(session_id);
            }
            return false;
        }
        return true;
    }
    st.last_len = cur_len;

    let mut parsed = jsonl::parse_file(&path, &entry.id);
    // Fill task parts' running/completed state from the child transcripts.
    jsonl::enrich_subagents(&mut parsed);
    // Tail background-task output files into their parts.
    jsonl::enrich_background_tasks(&mut parsed);
    // Keep re-reading output files while any background command is still running.
    st.has_pending_bg = jsonl::has_running_background_task(&parsed);
    let ts = now_ms();

    if let Some(title) = &parsed.title {
        engine.set_title(session_id, title, false);
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
    st.has_pending_sub = false;
    for aid in &parsed.subagent_ids {
        let (title, _) = sub_meta.get(aid).cloned().unwrap_or_default();
        engine.ensure_subagent_session(session_id, aid, &title, &dir);
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
            if st.emitted_sub.get(&key) != Some(&h) {
                events::emit_message(engine, &dir, aid, msg, ts);
                st.emitted_sub.insert(key, h);
            }
        }
        // A subagent counts as in-flight ONLY if its transcript is unfinished AND
        // still being written. A stale transcript means it finished without a
        // clean final message or was orphaned — never let that wedge the parent.
        let (done, _) = jsonl::subagent_completed(&sub);
        let running = !done && transcript_is_fresh(&sub_path);
        engine.set_busy(aid, running);
        if running {
            st.has_pending_sub = true;
        }
    }
    // Publish to the registry so the status poller keeps the session busy while a
    // subagent genuinely runs past the main agent's `state=done` — bounded by
    // liveness above so it can never pin the session busy forever.
    engine.set_subagent_pending(session_id, st.has_pending_sub);

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
        if st.emitted.get(&id) != Some(&h) {
            events::emit_message(engine, &dir, &entry.id, msg, ts);
            st.emitted.insert(id, h);
            any_new = true;
        }
    }

    // Stop tailing a session that has been idle and unchanged for a while
    // (the status poller still tracks it; reopening a turn re-spawns a tailer).
    // At 100ms/tick, 600 ticks ≈ 60s of quiet after idle.
    if any_new {
        st.idle_ticks = 0;
    } else if !entry.busy {
        st.idle_ticks += 1;
        if st.idle_ticks > 600 {
            if let Ok(mut t) = engine.tailers.lock() {
                t.remove(session_id);
            }
            return false;
        }
    } else {
        st.idle_ticks = 1; // busy: keep the skip-path counter primed but non-exiting
    }
    true
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
        // uuids we've observed actively running this process — so we only surface a
        // *failure* for a turn we actually saw start (never pre-existing failed agents
        // from earlier runs, which would otherwise spam an error on every opman restart).
        let mut seen_busy: std::collections::HashSet<String> = std::collections::HashSet::new();
        // uuids we've already surfaced a failure for (notify once per turn).
        let mut notified_failed: std::collections::HashSet<String> = std::collections::HashSet::new();
        loop {
            // Run the (blocking) CLI call off the async reactor.
            let agents = tokio::task::spawn_blocking(|| claude_cli::agents_json(None))
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();

            tick_status_poller(&engine, &agents, &mut absent, &mut seen_busy, &mut notified_failed);

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

/// Run one reconciliation pass of the status poller over an already-fetched agent list.
/// Behavior-preserving extraction of `spawn_status_poller`'s loop body (everything
/// between the `agents` fetch and the trailing sleep) so a single pass can be driven in
/// tests with a crafted `AgentInfo` list and the poller's carried-over debounce state.
pub(crate) fn tick_status_poller(
    engine: &Arc<ClaudeEngine>,
    agents: &[claude_cli::AgentInfo],
    absent: &mut HashMap<String, u32>,
    seen_busy: &mut std::collections::HashSet<String>,
    notified_failed: &mut std::collections::HashSet<String>,
) {
    // uuid → busy, and uuid → raw state (to detect failures distinctly from done).
    let mut busy_by_uuid: HashMap<String, bool> = HashMap::new();
    let mut state_by_uuid: HashMap<String, String> = HashMap::new();
    for a in agents {
        if !a.session_id.is_empty() {
            busy_by_uuid.insert(a.session_id.clone(), a.is_busy());
            if let Some(s) = &a.state {
                state_by_uuid.insert(a.session_id.clone(), s.clone());
            }
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

        // Just-aborted sessions: `claude stop` is graceful, so the agent (and a
        // just-killed subagent's still-fresh transcript) can read busy for a beat.
        // Force the session idle while it settles instead of bouncing it to busy.
        let raw_busy = busy_by_uuid.get(&uuid).copied().unwrap_or(false)
            || engine.subagent_pending(&id);
        if engine.abort_settling(&id, raw_busy) {
            absent.remove(&uuid);
            engine.set_busy(&id, false);
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
        if busy {
            seen_busy.insert(uuid.clone());
        }

        // Surface a hard agent failure (process/daemon died mid-turn) to the
        // frontend — but only for a turn we actually saw running this run, and
        // only once per turn.
        if state_by_uuid.get(&uuid).map(|s| s == "failed").unwrap_or(false)
            && seen_busy.contains(&uuid)
            && notified_failed.insert(uuid.clone())
        {
            engine.emit_system(
                &id,
                "error",
                "The background agent failed — its process or daemon ended mid-turn. Send a message to resume from where it left off.",
            );
        }

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
                engine.emit_queue_changed(&id);
                engine.clone().spawn_turn(id.clone(), text);
            }
        }
    }
}

#[cfg(test)]
#[path = "tailer_tests.rs"]
mod tailer_tests;

#[cfg(test)]
#[path = "tailer_tick_tests.rs"]
mod tailer_tick_tests;

#[cfg(test)]
#[path = "tailer_poller_tests.rs"]
mod tailer_poller_tests;

#[cfg(test)]
#[path = "tailer_reconcile_tests.rs"]
mod tailer_reconcile_tests;
