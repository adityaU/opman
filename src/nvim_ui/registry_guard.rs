//! Drop-safe publication of a UI session's Neovim socket.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::mcp::NvimSocketRegistry;

use super::error::{NvimUiError, Result};
use super::key::SessionKey;

pub struct RegistryGuard {
    registry: NvimSocketRegistry,
    key: (usize, String),
    path: PathBuf,
    removed: AtomicBool,
}

impl RegistryGuard {
    pub async fn publish(
        registry: NvimSocketRegistry,
        key: &SessionKey,
        path: &Path,
    ) -> Result<Self> {
        let key = (key.project_idx, key.session_id.clone());
        let path = path.to_path_buf();
        registry.write().await.insert(key.clone(), path.clone());
        Ok(Self {
            registry,
            key,
            path,
            removed: AtomicBool::new(false),
        })
    }

    pub async fn remove(&self) {
        if self.removed.swap(true, Ordering::AcqRel) {
            return;
        }
        remove_if_current(&self.registry, &self.key, &self.path).await;
    }
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        if self.removed.swap(true, Ordering::AcqRel) {
            return;
        }
        let registry = self.registry.clone();
        let key = self.key.clone();
        let path = self.path.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                remove_if_current(&registry, &key, &path).await;
            });
            return;
        }
        let mut entries = registry.blocking_write();
        if entries.get(&key).is_some_and(|current| current == &path) {
            entries.remove(&key);
        }
    }
}

async fn remove_if_current(registry: &NvimSocketRegistry, key: &(usize, String), path: &Path) {
    let mut entries = registry.write().await;
    if entries.get(key).is_some_and(|current| current == path) {
        entries.remove(key);
    }
}

impl From<std::io::Error> for NvimUiError {
    fn from(error: std::io::Error) -> Self {
        Self::Registry(error)
    }
}

#[cfg(test)]
#[path = "registry_guard_tests.rs"]
mod registry_guard_tests;
