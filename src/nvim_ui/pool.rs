//! Lazy keyed pool of embedded Neovim UI sessions.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::debug;

use super::error::Result;
use super::key::{SessionKey, UiSize};
use super::session::NvimSession;
use super::spawn::ConfigSource;
use crate::mcp::NvimSocketRegistry;

pub const MAX_SESSIONS: usize = 8;

#[derive(Default)]
struct Inner {
    sessions: HashMap<SessionKey, Arc<NvimSession>>,
}

pub struct NvimUiPool {
    registry: NvimSocketRegistry,
    config: ConfigSource,
    inner: Mutex<Inner>,
    ensure_gate: Mutex<()>,
}

impl NvimUiPool {
    pub fn new(registry: NvimSocketRegistry) -> Self {
        Self::with_config(registry, ConfigSource::UserConfig)
    }

    pub fn with_config(registry: NvimSocketRegistry, config: ConfigSource) -> Self {
        Self {
            registry,
            config,
            inner: Mutex::new(Inner::default()),
            ensure_gate: Mutex::new(()),
        }
    }

    /// Find or start the session for a key. A dead entry is never returned.
    pub async fn ensure(
        &self,
        key: SessionKey,
        project_dir: &Path,
        size: UiSize,
    ) -> Result<Arc<NvimSession>> {
        let _gate = self.ensure_gate.lock().await;
        let mut retired = Vec::new();
        {
            let mut inner = self.inner.lock().await;
            if let Some(existing) = inner.sessions.get(&key) {
                if existing.is_alive() {
                    existing.touch();
                    return Ok(existing.clone());
                }
                if let Some(session) = inner.sessions.remove(&key) {
                    retired.push(session);
                }
            }
            retired.extend(evict_oldest(&mut inner, &key));
        }
        shutdown_all(retired).await;

        let session = NvimSession::start_with_config(
            self.registry.clone(),
            key.clone(),
            project_dir,
            size,
            self.config.clone(),
        )
        .await?;
        self.inner
            .lock()
            .await
            .sessions
            .insert(key, session.clone());
        Ok(session)
    }

    /// Remove and shut down one session. The pool lock is not held while waiting.
    pub async fn evict(&self, key: &SessionKey) {
        let removed = self.inner.lock().await.sessions.remove(key);
        if let Some(session) = removed {
            session.shutdown().await;
        }
    }

    /// Shut down dead or idle sessions and return the number removed.
    pub async fn sweep(&self, idle: Duration) -> usize {
        let stale: Vec<_> = {
            let inner = self.inner.lock().await;
            inner
                .sessions
                .iter()
                .filter(|(_, session)| !session.is_alive() || session.idle_for() >= idle)
                .map(|(key, _)| key.clone())
                .collect()
        };
        for key in &stale {
            self.evict(key).await;
        }
        stale.len()
    }

    pub async fn shutdown_all(&self) {
        let sessions: Vec<_> = self
            .inner
            .lock()
            .await
            .sessions
            .drain()
            .map(|(_, session)| session)
            .collect();
        shutdown_all(sessions).await;
    }

    pub async fn len(&self) -> usize {
        self.inner.lock().await.sessions.len()
    }

    pub async fn get(&self, key: &SessionKey) -> Option<Arc<NvimSession>> {
        self.inner.lock().await.sessions.get(key).cloned()
    }
}

fn evict_oldest(inner: &mut Inner, requested: &SessionKey) -> Vec<Arc<NvimSession>> {
    if inner.sessions.len() < MAX_SESSIONS {
        return Vec::new();
    }
    let oldest = inner
        .sessions
        .iter()
        .filter(|(key, _)| *key != requested)
        .max_by_key(|(_, session)| session.idle_for())
        .map(|(key, _)| key.clone());
    oldest
        .and_then(|key| inner.sessions.remove(&key))
        .into_iter()
        .collect()
}

async fn shutdown_all(sessions: Vec<Arc<NvimSession>>) {
    for session in sessions {
        debug!(key = ?session.key(), "nvim ui: shutting down session");
        session.shutdown().await;
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod pool_tests;
