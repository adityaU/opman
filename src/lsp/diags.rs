//! Diagnostics arrive when the server feels like it; the HTTP endpoint asks for
//! them now.
//!
//! Servers push `textDocument/publishDiagnostics` on their own schedule —
//! rust-analyzer says nothing for tens of seconds on a cold project, then
//! publishes for every file at once. The GET bridging that gap keeps the latest
//! set per URI and, on the very first ask for a file, waits briefly for the
//! first publish so opening a file usually shows its errors instead of a
//! confident empty list.
//!
//! Two rules that are easy to get wrong and expensive to debug:
//!
//! * An empty array means *clean*, not *unknown*. Storing it as absence leaves
//!   a fixed file showing stale errors forever.
//! * Waiting is bounded and short. A wait that covers cold indexing would pile
//!   up axum tasks for a minute; the frontend polls, so the next poll gets them.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Notify;

/// How long the first request for a file waits for a server that has not
/// published yet.
pub const FIRST_PUBLISH_WAIT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct DiagStore {
    /// URI → the complete diagnostic set most recently published for it.
    by_uri: Mutex<HashMap<String, Vec<Value>>>,
    /// Woken on every publish so waiters can re-check.
    published: Notify,
}

impl DiagStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a publish. Replaces wholesale — each notification carries the
    /// full set for that URI.
    pub fn publish(&self, uri: String, diagnostics: Vec<Value>) {
        if let Ok(mut map) = self.by_uri.lock() {
            map.insert(uri, diagnostics);
        }
        self.published.notify_waiters();
    }

    /// The diagnostics known for `uri`. `None` means the server has never
    /// spoken about this file; `Some(vec![])` means it says the file is clean.
    pub fn get(&self, uri: &str) -> Option<Vec<Value>> {
        self.by_uri.lock().ok()?.get(uri).cloned()
    }

    /// Drop everything. Called when a server is replaced, since the new process
    /// has no memory of what the old one reported.
    pub fn clear(&self) {
        if let Ok(mut map) = self.by_uri.lock() {
            map.clear();
        }
    }

    /// Wait up to `budget` for a first publish covering `uri`, then give up and
    /// report an empty set rather than holding the connection.
    pub async fn wait_for(&self, uri: &str, budget: Duration) -> Vec<Value> {
        if let Some(found) = self.get(uri) {
            return found;
        }
        let deadline = tokio::time::Instant::now() + budget;
        loop {
            // Subscribe before re-checking, so a publish landing between the
            // check and the await cannot be missed.
            let waiter = self.published.notified();
            if let Some(found) = self.get(uri) {
                return found;
            }
            if tokio::time::timeout_at(deadline, waiter).await.is_err() {
                return self.get(uri).unwrap_or_default();
            }
        }
    }
}

#[cfg(test)]
#[path = "diags_tests.rs"]
mod diags_tests;
