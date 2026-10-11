//! The sessions a not-yet-started runner already has on disk.

use std::collections::HashSet;
use std::sync::Mutex;

use crate::app::SessionInfo;
use crate::session_disk::SessionStore;

/// A runner's on-disk session store, plus the ids it has listed so far.
///
/// Listing is what lets the sidebar show previous sessions before the runner exists. The
/// ids are remembered because a listed session is a promise: opening it must start the
/// runner that can render it, where a request about an id this runner never listed keeps
/// the old "nothing here yet" answer and starts nothing.
pub(super) struct DiskSessions {
    store: SessionStore,
    listed: Mutex<HashSet<String>>,
}

impl DiskSessions {
    pub(super) fn new(store: SessionStore) -> Self {
        Self {
            store,
            listed: Mutex::new(HashSet::new()),
        }
    }

    /// The store's sessions for `directory`, read without starting anything.
    pub(super) async fn list(&self, directory: &str) -> Vec<SessionInfo> {
        let sessions = self.store.list_async(directory).await;
        if let Ok(mut listed) = self.listed.lock() {
            listed.extend(sessions.iter().map(|session| session.id.clone()));
        }
        sessions
    }

    /// Whether `session_id` came from this store.
    pub(super) fn listed(&self, session_id: &str) -> bool {
        self.listed
            .lock()
            .map(|listed| listed.contains(session_id))
            .unwrap_or(false)
    }
}
