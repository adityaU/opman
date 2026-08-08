//! Session running status — one path shared by every runner.
//!
//! A turn belongs to exactly one runner, and only that runner knows whether it
//! is still going. Both the runner event streams and the fallback poller land
//! in [`WebStateHandle::set_session_running`], so a transition means the same
//! thing whichever of them saw it first and the idle side-effects (watchers,
//! missions, routines, kanban chaining) fire exactly once per turn.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::super::types::*;

/// How often the fallback sweep asks the runners who is running.
///
/// The sweep exists because event streams drop; it is the floor on how long a
/// stale spinner can survive, so it is deliberately short. Every runner but
/// opencode answers in-process, making this close to free.
const SWEEP_INTERVAL: Duration = Duration::from_secs(3);

/// How long a session stays running on the strength of a dispatched send alone.
///
/// A runner does not report a turn until it has accepted it. Without this the
/// sweep lands in the gap between "opman sent the prompt" and "the runner says
/// busy", reads idle, and bounces the composer out of its running state.
const DISPATCH_GRACE: Duration = Duration::from_secs(20);

/// Which way a session's running state moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Running {
    Busy,
    Idle,
}

/// What one sweep across the runners observed.
#[derive(Clone, Debug, Default)]
pub(crate) struct StatusSweep {
    /// Sessions some runner reports as running.
    pub(crate) running: HashSet<String>,
    /// Display names of the runners that answered. A runner absent from this
    /// set told us nothing this round, so its sessions keep the state they had.
    pub(crate) observed: HashSet<String>,
    /// Whether every runner asked actually answered.
    ///
    /// Sessions opman has not labelled — a subagent, or one created before it
    /// learned who owns it — cannot be attributed to a runner, so a complete
    /// sweep is the only thing that can speak for them.
    pub(crate) complete: bool,
}

impl StatusSweep {
    /// A sweep that has asked nobody yet. `complete` starts true so that
    /// merging the per-runner answers can only ever take it away.
    fn pending() -> Self {
        Self {
            complete: true,
            ..Self::default()
        }
    }

    fn absorb(&mut self, entries: Vec<(String, Option<HashSet<String>>)>) {
        for (runner, reported) in entries {
            let Some(reported) = reported else {
                self.complete = false;
                continue;
            };
            self.running.extend(reported);
            self.observed.insert(runner);
        }
    }

    /// Whether this sweep is entitled to retire `session_id`'s running state.
    fn can_retire(&self, session_id: &str, owner: Option<&String>) -> bool {
        match owner {
            Some(owner) => self.observed.contains(owner),
            None => self.complete && !self.observed.is_empty(),
        }
        .then(|| !self.running.contains(session_id))
        .unwrap_or(false)
    }
}

impl super::WebStateHandle {
    /// Record the one place a session's running state changes.
    ///
    /// Returns whether this was a real transition. Callers may fire freely: a
    /// repeated observation of the state a session is already in does nothing,
    /// which is what keeps the poller from re-running an idle hook the event
    /// stream already ran.
    pub(crate) async fn set_session_running(&self, session_id: &str, running: Running) -> bool {
        let changed = self.record_running(session_id, running).await;
        if !changed {
            return false;
        }
        match running {
            Running::Busy => self.cancel_watcher_timer(session_id).await,
            Running::Idle => {
                self.try_trigger_watcher(session_id).await;
                self.try_fire_idle_routines(session_id).await;
                self.try_advance_kanban_pipeline(session_id).await;
            }
        }
        true
    }

    /// Apply the state half of a transition and emit its events, holding the
    /// write lock for no longer than that. Split out so the idle side-effects —
    /// which take the lock themselves — run after it is released.
    async fn record_running(&self, session_id: &str, running: Running) -> bool {
        let mut state = self.inner.write().await;
        match running {
            Running::Busy => {
                // A session that is working again is no longer in its error state.
                if state.error_sessions.remove(session_id).is_some() {
                    let _ = self.event_tx.send(WebEvent::StateChanged);
                }
                if !state.busy_sessions.insert(session_id.to_string()) {
                    return false;
                }
                let _ = self.event_tx.send(WebEvent::SessionBusy {
                    session_id: session_id.to_string(),
                });
                true
            }
            Running::Idle => {
                state.turn_dispatch.remove(session_id);
                if !state.busy_sessions.remove(session_id) {
                    return false;
                }
                let _ = self.event_tx.send(WebEvent::SessionIdle {
                    session_id: session_id.to_string(),
                });
                mark_unseen_on_idle(&mut state, session_id, &self.event_tx);
                true
            }
        }
    }

    /// Hold a session running from the moment its prompt is dispatched.
    ///
    /// The runner has not accepted the turn yet, so nothing else can report it,
    /// and a UI that shows "idle" for that beat looks like the send was lost.
    /// The mark expires after [`DISPATCH_GRACE`] so a send that never reached a
    /// runner cannot strand the session as busy.
    pub async fn mark_turn_dispatched(&self, session_id: &str) {
        {
            let mut state = self.inner.write().await;
            state
                .turn_dispatch
                .insert(session_id.to_string(), Instant::now());
        }
        self.set_session_running(session_id, Running::Busy).await;
    }

    /// Count a finished turn that did not move the recorded state.
    ///
    /// A runner saying "idle" is authoritative about the turn ending even when
    /// opman never saw it start — an opman restart mid-turn is exactly that.
    /// Runners emit the edge, not a heartbeat, so this cannot double-count a
    /// turn [`set_session_running`] already counted.
    pub(crate) async fn note_untracked_idle(&self, session_id: &str) {
        let mut state = self.inner.write().await;
        state.turn_dispatch.remove(session_id);
        mark_unseen_on_idle(&mut state, session_id, &self.event_tx);
    }

    /// Drop a session's dispatch grace without asserting it is idle.
    ///
    /// Used when the turn stops being this session's — an abort, or a handoff
    /// that moved it elsewhere. The runners stay authoritative on what happens
    /// next; this only stops the grace from speaking for them.
    pub async fn mark_turn_settled(&self, session_id: &str) {
        let mut state = self.inner.write().await;
        state.turn_dispatch.remove(session_id);
    }

    /// Ask every runner, for every project, which sessions are running.
    pub(crate) async fn sweep_session_status(&self) -> StatusSweep {
        let Some(registry) = self.runner_registry.clone() else {
            return StatusSweep::default();
        };
        let directories = self.project_directories().await;
        let probes = directories
            .iter()
            .map(|directory| registry.status_all(directory));
        let mut sweep = StatusSweep::pending();
        for entries in futures::future::join_all(probes).await {
            sweep.absorb(entries);
        }
        sweep
    }

    /// Reconcile the sweep against the recorded state.
    ///
    /// Sessions gain running state from any runner that reports them. They lose
    /// it only when the runner that owns them actually answered and left them
    /// out — an unreachable runner is unobserved, not idle. Returns the
    /// `(became_busy, became_idle)` ids, for tests and tracing.
    pub(crate) async fn apply_status_sweep(
        &self,
        sweep: &StatusSweep,
    ) -> (Vec<String>, Vec<String>) {
        let (became_busy, became_idle) = {
            let state = self.inner.read().await;
            let became_busy: Vec<String> = sweep
                .running
                .iter()
                .filter(|id| !state.busy_sessions.contains(*id))
                .cloned()
                .collect();
            let cutoff = Instant::now() - DISPATCH_GRACE;
            let became_idle: Vec<String> = state
                .busy_sessions
                .iter()
                .filter(|id| {
                    state
                        .turn_dispatch
                        .get(*id)
                        .is_none_or(|dispatched| *dispatched <= cutoff)
                })
                .filter(|id| sweep.can_retire(id, state.session_runners.get(*id)))
                .cloned()
                .collect();
            (became_busy, became_idle)
        };

        for id in &became_busy {
            self.set_session_running(id, Running::Busy).await;
        }
        for id in &became_idle {
            self.set_session_running(id, Running::Idle).await;
        }
        (became_busy, became_idle)
    }

    /// One sweep-and-reconcile tick. Extracted so a single tick is testable.
    pub(crate) async fn status_sweep_once(&self) -> (Vec<String>, Vec<String>) {
        let sweep = self.sweep_session_status().await;
        self.apply_status_sweep(&sweep).await
    }

    /// Poll the runners for running status until the process exits.
    ///
    /// The runner event streams are the fast path; this is the one that makes a
    /// dropped stream, a runner restart, or a missed edge self-correct instead
    /// of leaving a session spinning forever.
    pub(super) fn spawn_status_poller(&self) {
        let handle = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(SWEEP_INTERVAL).await;
                handle.status_sweep_once().await;
            }
        });
    }
}

/// Count a finished turn as unseen when its session is not on screen.
///
/// Subagents are skipped, matching upstream opencode's `handleSessionIdle`:
/// a parent's own idle already stands for the work its children did.
fn mark_unseen_on_idle(
    state: &mut super::WebStateInner,
    session_id: &str,
    event_tx: &tokio::sync::broadcast::Sender<WebEvent>,
) {
    let is_subagent = state
        .projects
        .iter()
        .flat_map(|project| project.sessions.iter())
        .find(|session| session.id == session_id)
        .is_some_and(|session| !session.parent_id.is_empty());
    if is_subagent {
        return;
    }
    let is_active = state
        .projects
        .iter()
        .any(|project| project.active_session.as_deref() == Some(session_id));
    if is_active {
        return;
    }
    let count = state
        .unseen_sessions
        .entry(session_id.to_string())
        .or_insert(0);
    *count += 1;
    let _ = event_tx.send(WebEvent::SessionUnseen {
        session_id: session_id.to_string(),
        count: *count,
    });
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;
