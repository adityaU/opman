//! The embedded claude engine that is running right now, and how to stop it.
//!
//! The engine used to be started once and live until the process exited, so a `OnceLock`
//! and a handful of detached tasks were enough. A runner can now be disabled from the
//! settings page, which has to actually stop it — the HTTP listener, the status poller and
//! the reaper — and enabling it again starts a fresh engine that replaces this one.

use std::sync::{Arc, Mutex, RwLock};

use tokio::task::AbortHandle;

use super::ClaudeEngine;

/// The engine the PTY layer and the TUI resolve session ids against.
static ENGINE: RwLock<Option<Arc<ClaudeEngine>>> = RwLock::new(None);

/// Every background task the running engine owns.
static TASKS: Mutex<Vec<AbortHandle>> = Mutex::new(Vec::new());

/// The running engine, if one is up.
pub fn engine() -> Option<Arc<ClaudeEngine>> {
    match ENGINE.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Make `engine` the one [`engine`] answers with.
pub(super) fn publish(engine: &Arc<ClaudeEngine>) {
    match ENGINE.write() {
        Ok(mut guard) => *guard = Some(Arc::clone(engine)),
        Err(poisoned) => *poisoned.into_inner() = Some(Arc::clone(engine)),
    }
}

/// Hand a task to the shutdown path.
pub(super) fn track(task: AbortHandle) {
    match TASKS.lock() {
        Ok(mut tasks) => tasks.push(task),
        Err(poisoned) => poisoned.into_inner().push(task),
    }
}

/// Stop the embedded engine: its listener, poller and reaper.
///
/// Background `claude` agents a turn already launched are separate processes owned by the
/// Claude CLI, not by this engine, so a turn in flight finishes on its own; nothing here
/// would know how to resume it, and killing it would lose the user's work.
pub fn shutdown_embedded() {
    let tasks = match TASKS.lock() {
        Ok(mut tasks) => std::mem::take(&mut *tasks),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    for task in &tasks {
        task.abort();
    }
    match ENGINE.write() {
        Ok(mut guard) => *guard = None,
        Err(poisoned) => *poisoned.into_inner() = None,
    }
    tracing::info!(tasks = tasks.len(), "embedded claude engine stopped");
}
