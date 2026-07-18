use std::collections::HashSet;

use futures::future::join_all;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::api::ApiClient;
use crate::app::base_url;

use super::super::types::*;
use super::sse::run_opencode_sse;

impl super::WebStateHandle {
    pub(super) fn schedule_persist(&self) {
        let _ = self.persist_tx.send(());
    }

    /// Spawn a background worker that debounces DB writes.
    ///
    /// When `schedule_persist()` is called, this worker waits 150ms, drains
    /// duplicate signals, then snapshots the in-memory state and writes it
    /// to SQLite via `spawn_blocking`.
    pub(super) fn spawn_persist_worker(&self, mut persist_rx: mpsc::UnboundedReceiver<()>) {
        let handle = self.clone();

        tokio::spawn(async move {
            while persist_rx.recv().await.is_some() {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                while persist_rx.try_recv().is_ok() {}

                match handle.persist_snapshot_once().await {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => warn!("failed to persist assistant state to DB: {}", err),
                    Err(err) => warn!("persist worker join error: {}", err),
                }
            }
        });
    }

    /// Snapshot the mutable assistant state and write it to SQLite via
    /// `spawn_blocking`. Returns the raw `spawn_blocking` result so the caller can
    /// distinguish a DB error (`Ok(Err)`) from a join error (`Err`). Extracted from
    /// the persist worker loop so it can be driven once in tests.
    pub(crate) async fn persist_snapshot_once(
        &self,
    ) -> std::result::Result<
        std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>,
        tokio::task::JoinError,
    > {
        let state = self.inner.read().await;
        let missions: Vec<_> = state.missions.values().cloned().collect();
        let memory: Vec<_> = state.personal_memory.values().cloned().collect();
        let autonomy = state.autonomy_settings.clone();
        let routines: Vec<_> = state.routines.values().cloned().collect();
        let routine_runs = state.routine_runs.clone();
        let delegated: Vec<_> = state.delegated_work.values().cloned().collect();
        let workspaces: Vec<_> = state.workspaces.values().cloned().collect();
        let signals = state.signals.clone();
        drop(state);

        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            super::db_sync::sync_all(
                &db, &missions, &memory, &autonomy, &routines,
                &routine_runs, &delegated, &workspaces, &signals,
            )
        })
        .await
    }

    // ── Background tasks ────────────────────────────────────────────

    /// Poll `GET /session` for each project every 30 seconds.
    ///
    /// On startup the poller retries aggressively (100ms → 200ms → …) so that
    /// sessions are available for the very first `/api/state` request from the
    /// frontend, eliminating the previous 2-second race window.
    pub(super) fn spawn_session_poller(&self) {
        let handle = self.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let client = ApiClient::new();

            // Eagerly poll with exponential back-off so sessions are ready
            // before the first frontend /api/state request arrives.
            {
                let mut delay_ms: u64 = 100;
                for attempt in 1..=8 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    let base = base_url().to_string();
                    if handle.session_poll_startup_once(&client, &base).await {
                        let _ = event_tx.send(WebEvent::StateChanged);
                        debug!("initial session poll succeeded on attempt {attempt}");
                        break;
                    }
                    delay_ms = (delay_ms * 2).min(2000);
                }
            }

            loop {
                let base = base_url().to_string();
                handle.session_poll_iter_once(&client, &base).await;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }

    /// One eager-startup poll attempt: fetch sessions for each project and, on the
    /// first success, hydrate `active_session` + the session list. Returns whether
    /// any project fetch succeeded. Extracted from `spawn_session_poller`.
    pub(crate) async fn session_poll_startup_once(&self, client: &ApiClient, base: &str) -> bool {
        let project_paths: Vec<(usize, String)> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.path.to_string_lossy().to_string()))
                .collect()
        };
        let mut any_ok = false;
        for (idx, dir) in &project_paths {
            if let Ok(sessions) = client.fetch_sessions(base, dir).await {
                any_ok = true;
                let filtered: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| s.directory == *dir)
                    .collect();
                let mut state = self.inner.write().await;
                if let Some(project) = state.projects.get_mut(*idx) {
                    if project.active_session.is_none() {
                        if let Some(first) = filtered.first() {
                            project.active_session = Some(first.id.clone());
                        }
                    }
                    project.sessions = filtered;
                }
            }
        }
        any_ok
    }

    /// One recurring poll iteration: refresh each project's session list (marking
    /// `StateChanged` only on a real diff) and reconcile busy/idle transitions,
    /// firing watcher/mission/routine side-effects. Extracted from
    /// `spawn_session_poller` so a single iteration can be driven in tests.
    pub(crate) async fn session_poll_iter_once(&self, client: &ApiClient, base: &str) {
        // Snapshot project paths
        let project_paths: Vec<(usize, String)> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.path.to_string_lossy().to_string()))
                .collect()
        };

        let mut changed = false;

        let fetches = project_paths.iter().map(|(idx, dir)| {
            let base = base.to_string();
            let dir = dir.clone();
            let idx = *idx;
            async move {
                // Run both fetches concurrently within each project
                let (sessions, status_map) = tokio::join!(
                    client.fetch_sessions(&base, &dir),
                    client.fetch_session_status(&base, &dir)
                );
                (idx, dir, sessions.ok(), status_map.ok())
            }
        });

        let results = join_all(fetches).await;

        let mut aggregated_busy = HashSet::new();

        for (idx, dir, sessions, status_map) in results {
            if let Some(sessions) = sessions {
                let mut state = self.inner.write().await;
                if let Some(project) = state.projects.get_mut(idx) {
                    let filtered: Vec<_> = sessions
                        .into_iter()
                        .filter(|s| s.directory == dir)
                        .collect();
                    // NOTE: Do NOT auto-assign active_session here.
                    // The recurring poller should never change the
                    // user's active session; only the startup hydration
                    // (above) and explicit user actions may do so.
                    // Only mark changed if the session list actually differs
                    let sessions_differ = {
                        if project.sessions.len() != filtered.len() {
                            true
                        } else {
                            project.sessions.iter().zip(filtered.iter()).any(|(a, b)| {
                                a.id != b.id || a.title != b.title || a.time.updated != b.time.updated
                            })
                        }
                    };
                    if sessions_differ {
                        project.sessions = filtered;
                        changed = true;
                    }
                }
            }

            if let Some(status_map) = status_map {
                for (session_id, status) in &status_map {
                    if status != "idle" {
                        aggregated_busy.insert(session_id.clone());
                    }
                }
            }
        }

        // Collect transitions before writing so we can fire side-effects
        // outside the lock.
        let (newly_busy, newly_idle) = {
            let mut state = self.inner.write().await;
            let mut n_busy = Vec::new();
            let mut n_idle = Vec::new();
            for id in &aggregated_busy {
                if !state.busy_sessions.contains(id) {
                    n_busy.push(id.clone());
                    let _ = self.event_tx.send(WebEvent::SessionBusy {
                        session_id: id.clone(),
                    });
                }
            }
            for id in state.busy_sessions.iter() {
                if !aggregated_busy.contains(id) {
                    n_idle.push(id.clone());
                    let _ = self.event_tx.send(WebEvent::SessionIdle {
                        session_id: id.clone(),
                    });
                }
            }
            state.busy_sessions = aggregated_busy;
            (n_busy, n_idle)
        };

        // Fire side-effects for transitions detected by the poller
        // (mirrors what the SSE handler does on real-time events).
        for sid in &newly_idle {
            self.try_trigger_watcher(sid).await;
            self.try_advance_mission(sid).await;
            self.try_fire_idle_routines(sid).await;
        }
        for sid in &newly_busy {
            self.cancel_watcher_timer(sid).await;
        }

        if changed {
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
    }

    /// Listen to the opencode server's SSE `/event` stream to capture
    /// session stats (cost/tokens) from `message.updated` events.
    ///
    /// Spawns one SSE connection per project directory. Every 2 minutes the
    /// connections are torn down and re-established so we pick up any new
    /// projects and drop stale connections. Individual connections also
    /// self-terminate via the heartbeat watchdog in `run_opencode_sse` if
    /// the upstream goes silent for >60s.
    pub(super) fn spawn_opencode_sse_listener(&self) {
        let handle = self.clone();

        tokio::spawn(async move {
            // Wait for server to be ready
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

            loop {
                // Cancel previous SSE tasks before spawning new ones
                for h in handles.drain(..) {
                    h.abort();
                }

                let base = base_url().to_string();
                handles = handle.opencode_sse_reconnect_once(&base).await;

                // Reconnect loop: check every 2 minutes if we need to restart
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            }
        });
    }

    /// Snapshot the current project dirs and spawn one `run_opencode_sse` task per
    /// dir, returning their join handles. Extracted from `spawn_opencode_sse_listener`
    /// so a single reconnect tick can be driven in tests. The caller is responsible
    /// for aborting the previous batch of handles before calling this.
    pub(crate) async fn opencode_sse_reconnect_once(
        &self,
        base: &str,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        // Get all project dirs to listen for
        let project_dirs: Vec<String> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .map(|p| p.path.to_string_lossy().to_string())
                .collect()
        };

        let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        // Connect SSE for each project
        for dir in &project_dirs {
            let handle_clone = self.clone();
            let dir_clone = dir.clone();
            let base_clone = base.to_string();

            let h = tokio::spawn(async move {
                if let Err(e) =
                    run_opencode_sse(&handle_clone, &base_clone, &dir_clone).await
                {
                    debug!("OpenCode SSE stream error for {}: {}", dir_clone, e);
                }
            });
            handles.push(h);
        }
        handles
    }
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod background_tests;

#[cfg(test)]
#[path = "background_iteration_tests.rs"]
mod background_iteration_tests;

#[cfg(test)]
#[path = "background_upstream_tests.rs"]
mod background_upstream_tests;
