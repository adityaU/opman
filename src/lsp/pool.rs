//! The set of running language servers, one per (project root, language).
//!
//! Keying on the root rather than the file is the whole point: every `.rs` file
//! in a workspace shares one rust-analyzer, which is what makes a
//! gigabyte-scale process affordable. Two invariants keep it honest —
//! `ensure` returns an `Arc` clone and never the map entry, so a caller cannot
//! drop the child across an await; and the spawn lock is held only across
//! `Command::spawn`, never across the handshake, so a slow-starting server does
//! not block requests for a different one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use tokio::sync::Mutex;
use tracing::debug;

use super::detect::{self, LanguageId, ServerSpec};
use super::server::LspServer;

/// Servers cost real memory; refuse to accumulate them without bound.
const MAX_SERVERS: usize = 12;
/// How long a "binary not installed" answer is trusted, so a polling frontend
/// does not fork-and-fail several times a second.
const MISSING_BINARY_TTL: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ServerKey {
    pub root: PathBuf,
    pub language: LanguageId,
}

#[derive(Default)]
struct Inner {
    servers: HashMap<ServerKey, Arc<LspServer>>,
    /// Commands we recently failed to start, and when.
    missing: HashMap<&'static str, Instant>,
}

#[derive(Default)]
pub struct LspPool {
    inner: Mutex<Inner>,
}

/// Everything an operation needs to reach the right server.
pub struct Resolved {
    pub server: Arc<LspServer>,
    pub spec: ServerSpec,
    pub key: ServerKey,
}

impl Resolved {
    /// Whether this server was rooted at `expected` — the check that catches a
    /// misdetected project, which produces confident nonsense rather than an
    /// error.
    pub fn root_is(&self, expected: &Path) -> bool {
        self.key.root == expected
    }
}

impl LspPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Find or start the server for `file`. `Ok(None)` means there is simply no
    /// LSP for this file — an unknown extension, or a server we know of but
    /// which is not installed. Only a genuine failure is an `Err`.
    pub async fn resolve(&self, file: &Path, project_dir: &Path) -> Result<Option<Resolved>> {
        let Some(language) = detect::language_for(file) else {
            return Ok(None);
        };
        let Some(spec) = detect::spec_for(language) else {
            return Ok(None);
        };

        let root = detect::project_root(file, project_dir, spec.roots);
        let key = ServerKey {
            root: root.clone(),
            language: spec.language,
        };

        let mut inner = self.inner.lock().await;

        if let Some(existing) = inner.servers.get(&key) {
            if existing.is_alive() {
                existing.touch();
                let server = existing.clone();
                drop(inner);
                return Ok(Some(Resolved { server, spec, key }));
            }
            debug!(?key, "lsp: server died, replacing");
            inner.servers.remove(&key);
        }

        if let Some(failed_at) = inner.missing.get(spec.command) {
            if failed_at.elapsed() < MISSING_BINARY_TTL {
                return Ok(None);
            }
            inner.missing.remove(spec.command);
        }

        let Some(binary) = detect::resolve_binary(spec.command) else {
            inner.missing.insert(spec.command, Instant::now());
            debug!(command = spec.command, "lsp: server binary not installed");
            return Ok(None);
        };

        Self::evict_oldest_if_full(&mut inner);

        let server = match LspServer::spawn(&spec, &binary, &root) {
            Ok(server) => server,
            Err(e) => {
                inner.missing.insert(spec.command, Instant::now());
                bail!("could not start {}: {e}", spec.command);
            }
        };
        inner.servers.insert(key.clone(), server.clone());
        debug!(?root, language = spec.language, "lsp: server started");

        Ok(Some(Resolved { server, spec, key }))
    }

    /// Drop a server, shutting it down off the lock.
    pub async fn evict(&self, key: &ServerKey) {
        let removed = self.inner.lock().await.servers.remove(key);
        if let Some(server) = removed {
            server.shutdown().await;
        }
    }

    /// Shut down servers untouched for longer than `idle`. Returns how many.
    pub async fn sweep(&self, idle: Duration) -> usize {
        let stale: Vec<_> = {
            let inner = self.inner.lock().await;
            inner
                .servers
                .iter()
                .filter(|(_, server)| !server.is_alive() || server.idle_for() >= idle)
                .map(|(key, _)| key.clone())
                .collect()
        };
        for key in &stale {
            self.evict(key).await;
        }
        stale.len()
    }

    /// Shut everything down — used when the process is going away.
    pub async fn shutdown_all(&self) {
        let servers: Vec<_> = self
            .inner
            .lock()
            .await
            .servers
            .drain()
            .map(|(_, s)| s)
            .collect();
        for server in servers {
            server.shutdown().await;
        }
    }

    fn evict_oldest_if_full(inner: &mut Inner) {
        if inner.servers.len() < MAX_SERVERS {
            return;
        }
        let oldest = inner
            .servers
            .iter()
            .max_by_key(|(_, server)| server.idle_for())
            .map(|(key, _)| key.clone());
        if let Some(key) = oldest {
            debug!(?key, "lsp: pool full, evicting least recently used");
            // Dropping the Arc kills the child via kill_on_drop; a graceful
            // shutdown would need an await we cannot take under this lock.
            inner.servers.remove(&key);
        }
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod pool_tests;
