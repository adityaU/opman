//! A swappable registry, so adding or toggling a server takes effect without a restart.
//!
//! Engines hold a [`RegistryHandle`] rather than an `Arc<McpRegistry>` and call
//! [`RegistryHandle::current`] each time they build a payload. Reads are an `Arc` clone
//! under a short read lock — the injection path never holds the lock across a render, and
//! never blocks on a reload.
//!
//! One runner is inherently exempt: OpenCode's configuration is handed to `opencode
//! serve` once at spawn, so a change there needs that process restarted. Every other
//! runner rebuilds its payload per session or per turn and picks changes up immediately.

use std::sync::{Arc, RwLock};

use super::{BuiltinFlags, McpRegistry};

/// A registry that can be replaced underneath its readers.
#[derive(Clone, Debug)]
pub struct RegistryHandle {
    inner: Arc<RwLock<Arc<McpRegistry>>>,
    flags: BuiltinFlags,
}

impl Default for RegistryHandle {
    fn default() -> Self {
        Self::new(Arc::new(McpRegistry::default()), BuiltinFlags::default())
    }
}

impl RegistryHandle {
    pub fn new(registry: Arc<McpRegistry>, flags: BuiltinFlags) -> Self {
        Self {
            inner: Arc::new(RwLock::new(registry)),
            flags,
        }
    }

    /// Load `mcp.json` now and hold the result.
    pub fn load(flags: BuiltinFlags) -> Self {
        Self::new(Arc::new(McpRegistry::load(flags)), flags)
    }

    /// The registry as of this moment.
    ///
    /// A poisoned lock returns the value anyway: a panic in an unrelated reader must not
    /// take every runner's MCP configuration down with it.
    pub fn current(&self) -> Arc<McpRegistry> {
        match self.inner.read() {
            Ok(guard) => Arc::clone(&guard),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        }
    }

    /// Re-read `mcp.json` and swap it in. Returns the new registry.
    ///
    /// Sessions created after this see the new set; sessions already running see it on
    /// their next turn, because Claude and Codex rebuild their payload per turn.
    pub fn reload(&self) -> Arc<McpRegistry> {
        let next = Arc::new(McpRegistry::load(self.flags));
        self.replace(Arc::clone(&next));
        next
    }

    pub fn replace(&self, registry: Arc<McpRegistry>) {
        match self.inner.write() {
            Ok(mut guard) => *guard = registry,
            Err(poisoned) => *poisoned.into_inner() = registry,
        }
    }

    pub fn flags(&self) -> BuiltinFlags {
        self.flags
    }
}

#[cfg(test)]
#[path = "handle_tests.rs"]
mod handle_tests;
