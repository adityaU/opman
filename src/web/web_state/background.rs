use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::app::base_url_ready;

#[cfg(test)]
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
        let memory: Vec<_> = state.personal_memory.values().cloned().collect();
        let autonomy = state.autonomy_settings.clone();
        let routines: Vec<_> = state.routines.values().cloned().collect();
        let routine_runs = state.routine_runs.clone();
        drop(state);

        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            super::db_sync::sync_all(&db, &memory, &autonomy, &routines, &routine_runs)
        })
        .await
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

                let base = base_url_ready().await.to_string();
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
                if let Err(e) = run_opencode_sse(&handle_clone, &base_clone, &dir_clone).await {
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
