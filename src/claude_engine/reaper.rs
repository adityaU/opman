//! Background-agent reaper for the Claude engine.
//!
//! opman spawns a fresh `claude --bg` agent per turn, and claude keeps every finished
//! agent — plus its daemon/spare/pty-host helpers — warm indefinitely. Across many turns
//! and sessions these pile up (200+ idle agents holding GBs of RSS on a busy host) and
//! slow opman over time. This reaper periodically stops the agents opman no longer needs:
//!
//!   * **superseded** — an older turn in a session whose current turn is a newer UUID.
//!   * **untracked**  — belongs to no registry session (abandoned / throwaway runs).
//!   * **stale-idle** — a session's *current* agent, idle beyond a TTL.
//!
//! Reaping is safe: `claude --bg --resume <uuid>` respawns from the on-disk transcript,
//! and the web UI renders from that same transcript — a stopped agent loses nothing but
//! the warm process. Busy agents, in-flight subagents, mid-dispatch sessions, and agents
//! younger than a short grace window are never touched.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, info};

use super::{claude_cli, now_ms, ClaudeEngine};

/// How often the reaper sweeps.
const REAP_INTERVAL: Duration = Duration::from_secs(60);

/// Default idle TTL for a session's current agent before it is reaped.
const DEFAULT_TTL_SECS: u64 = 300;

/// Never reap an agent younger than this — a just-spawned turn may not be recorded in the
/// registry yet, and racing it to a stop would kill a live turn.
const MIN_AGE_MS: u64 = 30_000;

/// `OPMAN_CLAUDE_REAP=0` disables the reaper entirely.
fn enabled() -> bool {
    std::env::var("OPMAN_CLAUDE_REAP")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Idle TTL in millis, overridable via `OPMAN_CLAUDE_AGENT_TTL_SECS`.
fn ttl_ms() -> u64 {
    std::env::var("OPMAN_CLAUDE_AGENT_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(DEFAULT_TTL_SECS)
        * 1000
}

/// Spawn the periodic reaper. First tick fires immediately, clearing any backlog.
pub fn spawn_reaper(engine: Arc<ClaudeEngine>) {
    if !enabled() {
        info!("claude agent reaper disabled (OPMAN_CLAUDE_REAP=0)");
        return;
    }
    tokio::spawn(async move {
        let mut ticker = interval(REAP_INTERVAL);
        loop {
            ticker.tick().await;
            let n = reap_once(&engine).await;
            if n > 0 {
                info!(reaped = n, "reaped stale claude background agents");
            }
        }
    });
}

/// The live target a session's current turn resumes into, plus the facts that decide
/// whether it is safe to reap.
#[derive(Debug, Clone)]
struct CurrentTarget {
    session_id: String,
    updated_ms: u64,
    /// Busy, mid-dispatch, or has an in-flight subagent — never reap.
    protected: bool,
}

/// One agent selected for reaping.
#[derive(Debug, Clone, PartialEq)]
struct ReapTarget {
    /// `claude stop <short_id>` target.
    short_id: String,
    /// Session whose `short_id` should be cleared (only for a current-target reap).
    clear_session: Option<String>,
    reason: &'static str,
}

/// Run a single reap sweep. Returns the number of agents stopped.
pub async fn reap_once(engine: &Arc<ClaudeEngine>) -> usize {
    let agents = tokio::task::spawn_blocking(|| claude_cli::agents_json(None))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    if agents.is_empty() {
        return 0;
    }

    let now = now_ms();
    let current = engine.reap_snapshot();
    let plan = build_plan(&agents, &current, now, ttl_ms());
    if plan.is_empty() {
        return 0;
    }

    // Clear short_ids first so the attach pane stops targeting agents we're about to kill.
    let to_clear: Vec<String> = plan
        .iter()
        .filter_map(|t| t.clear_session.clone())
        .collect();
    engine.clear_short_ids(&to_clear);

    // Stop the agents off the async reactor (each `claude stop` shells out).
    let shorts: Vec<String> = plan.iter().map(|t| t.short_id.clone()).collect();
    for t in &plan {
        debug!(short_id = %t.short_id, reason = t.reason, "reaping claude agent");
    }
    let count = shorts.len();
    let _ = tokio::task::spawn_blocking(move || {
        for s in shorts {
            let _ = claude_cli::stop(&s);
        }
    })
    .await;
    count
}

/// Classify every background agent against the registry snapshot — the pure core, split
/// out so it can be unit-tested without a live engine or `claude` binary.
fn build_plan(
    agents: &[claude_cli::AgentInfo],
    current: &HashMap<String, CurrentTarget>,
    now_ms: u64,
    ttl_ms: u64,
) -> Vec<ReapTarget> {
    let mut plan = Vec::new();
    for a in agents {
        // Only background agents; interactive REPLs are the user's, not ours.
        if a.kind != "background" {
            continue;
        }
        if a.id.is_empty() {
            continue;
        }
        // Actively working (or an in-flight subagent reporting working) — keep.
        if a.is_busy() {
            continue;
        }
        // Too young: a just-spawned turn may not be in the registry yet.
        if now_ms.saturating_sub(a.started_at) < MIN_AGE_MS {
            continue;
        }

        match current.get(&a.session_id) {
            Some(t) => {
                // A session's live resume target: reap only once idle past the TTL.
                if t.protected {
                    continue;
                }
                if now_ms.saturating_sub(t.updated_ms) < ttl_ms {
                    continue;
                }
                plan.push(ReapTarget {
                    short_id: a.id.clone(),
                    clear_session: Some(t.session_id.clone()),
                    reason: "stale-idle",
                });
            }
            None => {
                // Superseded lineage turn or a fully untracked agent — always reapable.
                plan.push(ReapTarget {
                    short_id: a.id.clone(),
                    clear_session: None,
                    reason: "superseded-or-untracked",
                });
            }
        }
    }
    plan
}

impl ClaudeEngine {
    /// Snapshot the current resume target of every (non-subagent) session, keyed by its
    /// claude UUID, with the facts the reaper needs to decide safety.
    fn reap_snapshot(&self) -> HashMap<String, CurrentTarget> {
        let dispatching = self
            .dispatching
            .lock()
            .map(|d| d.clone())
            .unwrap_or_default();
        let Ok(g) = self.reg.lock() else {
            return HashMap::new();
        };
        let mut map = HashMap::with_capacity(g.sessions.len());
        for s in g.sessions.values() {
            if s.is_subagent {
                continue;
            }
            let Some(uuid) = &s.claude_session_id else {
                continue;
            };
            let protected = s.busy || s.subagent_pending || dispatching.contains(&s.id);
            map.insert(
                uuid.clone(),
                CurrentTarget {
                    session_id: s.id.clone(),
                    updated_ms: s.updated,
                    protected,
                },
            );
        }
        map
    }

    /// Drop the cached `short_id` for the given sessions (their agent was reaped). The
    /// resume UUID is kept, so the next prompt respawns a fresh agent from the transcript.
    fn clear_short_ids(&self, session_ids: &[String]) {
        if session_ids.is_empty() {
            return;
        }
        let mut changed = false;
        if let Ok(mut g) = self.reg.lock() {
            for id in session_ids {
                if let Some(e) = g.sessions.get_mut(id) {
                    if e.short_id.take().is_some() {
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_cli::AgentInfo;

    fn agent(id: &str, uuid: &str, state: &str, started_at: u64) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            session_id: uuid.to_string(),
            cwd: "/proj".to_string(),
            kind: "background".to_string(),
            state: Some(state.to_string()),
            status: None,
            name: String::new(),
            started_at,
        }
    }

    fn target(session_id: &str, updated_ms: u64, protected: bool) -> CurrentTarget {
        CurrentTarget {
            session_id: session_id.to_string(),
            updated_ms,
            protected,
        }
    }

    const NOW: u64 = 10_000_000;
    const TTL: u64 = 300_000;
    // Old enough to clear the MIN_AGE grace window.
    const OLD: u64 = NOW - MIN_AGE_MS - 1;

    #[test]
    fn untracked_idle_agent_is_reaped() {
        let agents = vec![agent("aa", "uuid-untracked", "done", OLD)];
        let plan = build_plan(&agents, &HashMap::new(), NOW, TTL);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].short_id, "aa");
        assert!(plan[0].clear_session.is_none());
    }

    #[test]
    fn busy_agent_is_never_reaped() {
        let agents = vec![agent("aa", "u", "working", OLD)];
        assert!(build_plan(&agents, &HashMap::new(), NOW, TTL).is_empty());
    }

    #[test]
    fn young_agent_is_spared() {
        let agents = vec![agent("aa", "u", "done", NOW - 1_000)];
        assert!(build_plan(&agents, &HashMap::new(), NOW, TTL).is_empty());
    }

    #[test]
    fn current_target_within_ttl_is_kept() {
        let mut cur = HashMap::new();
        cur.insert("u".to_string(), target("ses_1", NOW - 1_000, false));
        let agents = vec![agent("aa", "u", "done", OLD)];
        assert!(build_plan(&agents, &cur, NOW, TTL).is_empty());
    }

    #[test]
    fn current_target_past_ttl_is_reaped_and_cleared() {
        let mut cur = HashMap::new();
        cur.insert("u".to_string(), target("ses_1", NOW - TTL - 1, false));
        let agents = vec![agent("aa", "u", "done", OLD)];
        let plan = build_plan(&agents, &cur, NOW, TTL);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].clear_session.as_deref(), Some("ses_1"));
        assert_eq!(plan[0].reason, "stale-idle");
    }

    #[test]
    fn protected_current_target_is_kept_even_past_ttl() {
        let mut cur = HashMap::new();
        cur.insert("u".to_string(), target("ses_1", NOW - TTL - 1, true));
        let agents = vec![agent("aa", "u", "done", OLD)];
        assert!(build_plan(&agents, &cur, NOW, TTL).is_empty());
    }

    #[test]
    fn interactive_agents_are_ignored() {
        let mut a = agent("aa", "u", "done", OLD);
        a.kind = "interactive".to_string();
        assert!(build_plan(&[a], &HashMap::new(), NOW, TTL).is_empty());
    }
}

#[cfg(test)]
#[path = "reaper_tests.rs"]
mod reaper_tests;
