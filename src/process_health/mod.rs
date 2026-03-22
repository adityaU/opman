//! Process health monitoring and runtime mitigation module.
//!
//! Provides containment, cleanup, and watchdog capabilities for opencode
//! processes managed by opman. Exposes a `HealthHandle` that can be
//! embedded in `ServerState` for API access.
//!
//! **Excluded by design:** process restart (user-specified constraint).

pub mod orphan_cleanup;
pub mod port_cleanup;
pub mod temp_cleanup;
pub mod watchdog;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

// ── Mitigation identifiers ──────────────────────────────────────────

/// All toggleable mitigations (except restart, which is excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mitigation {
    OrphanCleanup,
    PortCleanup,
    TempCleanup,
    FdWatchdog,
    MemoryWatchdog,
    ConnectionWatchdog,
}

impl Mitigation {
    pub const ALL: &'static [Mitigation] = &[
        Self::OrphanCleanup,
        Self::PortCleanup,
        Self::TempCleanup,
        Self::FdWatchdog,
        Self::MemoryWatchdog,
        Self::ConnectionWatchdog,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::OrphanCleanup => "Orphan Process Cleanup",
            Self::PortCleanup => "Port/Socket Cleanup",
            Self::TempCleanup => "Temp File Cleanup",
            Self::FdWatchdog => "File Descriptor Watchdog",
            Self::MemoryWatchdog => "Memory Pressure Watchdog",
            Self::ConnectionWatchdog => "Connection Limit Watchdog",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrphanCleanup => "orphan_cleanup",
            Self::PortCleanup => "port_cleanup",
            Self::TempCleanup => "temp_cleanup",
            Self::FdWatchdog => "fd_watchdog",
            Self::MemoryWatchdog => "memory_watchdog",
            Self::ConnectionWatchdog => "connection_watchdog",
        }
    }
}

// ── Audit log ───────────────────────────────────────────────────────

/// A single entry in the mitigation audit log.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub mitigation: Mitigation,
    pub action: String,
    pub detail: String,
    pub success: bool,
}

impl AuditEntry {
    pub fn now(mitigation: Mitigation, action: &str, detail: &str, success: bool) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            mitigation,
            action: action.to_string(),
            detail: detail.to_string(),
            success,
        }
    }
}

/// Ring buffer for audit entries (bounded to avoid unbounded growth).
const MAX_AUDIT_ENTRIES: usize = 500;

#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn push(&mut self, entry: AuditEntry) {
        if self.entries.len() >= MAX_AUDIT_ENTRIES {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn recent(&self, n: usize) -> &[AuditEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }
}

// ── Mitigation config ───────────────────────────────────────────────

/// Per-mitigation enabled state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationConfig {
    pub orphan_cleanup: bool,
    pub port_cleanup: bool,
    pub temp_cleanup: bool,
    pub fd_watchdog: bool,
    pub memory_watchdog: bool,
    pub connection_watchdog: bool,
}

impl Default for MitigationConfig {
    fn default() -> Self {
        Self {
            orphan_cleanup: true,
            port_cleanup: true,
            temp_cleanup: true,
            fd_watchdog: true,
            memory_watchdog: true,
            connection_watchdog: true,
        }
    }
}

impl MitigationConfig {
    pub fn is_enabled(&self, m: Mitigation) -> bool {
        match m {
            Mitigation::OrphanCleanup => self.orphan_cleanup,
            Mitigation::PortCleanup => self.port_cleanup,
            Mitigation::TempCleanup => self.temp_cleanup,
            Mitigation::FdWatchdog => self.fd_watchdog,
            Mitigation::MemoryWatchdog => self.memory_watchdog,
            Mitigation::ConnectionWatchdog => self.connection_watchdog,
        }
    }

    pub fn set_enabled(&mut self, m: Mitigation, enabled: bool) {
        match m {
            Mitigation::OrphanCleanup => self.orphan_cleanup = enabled,
            Mitigation::PortCleanup => self.port_cleanup = enabled,
            Mitigation::TempCleanup => self.temp_cleanup = enabled,
            Mitigation::FdWatchdog => self.fd_watchdog = enabled,
            Mitigation::MemoryWatchdog => self.memory_watchdog = enabled,
            Mitigation::ConnectionWatchdog => self.connection_watchdog = enabled,
        }
    }
}

// ── Live snapshot ───────────────────────────────────────────────────

/// Runtime snapshot of current health metrics.
#[derive(Debug, Clone, Serialize, Default)]
pub struct HealthSnapshot {
    pub orphan_pids: Vec<u32>,
    pub tracked_ports: Vec<PortRecord>,
    pub tracked_temp_files: Vec<String>,
    pub open_fds: Option<u64>,
    pub fd_limit: Option<u64>,
    pub memory_rss_bytes: Option<u64>,
    pub tcp_connections: Option<u32>,
}

/// A tracked port/socket record.
#[derive(Debug, Clone, Serialize)]
pub struct PortRecord {
    pub port: u16,
    pub pid: u32,
    pub state: String,
}

// ── Shared handle ───────────────────────────────────────────────────

/// Thread-safe handle to the process health subsystem.
/// Cloneable, cheap (Arc-wrapped interior mutability).
#[derive(Clone)]
pub struct HealthHandle {
    inner: Arc<RwLock<HealthInner>>,
    /// Notifies the watchdog loop when config changes.
    pub(crate) notify_tx: broadcast::Sender<()>,
}

struct HealthInner {
    config: MitigationConfig,
    audit: AuditLog,
    snapshot: HealthSnapshot,
}

impl HealthHandle {
    /// Create a new handle with default config and start the background watchdog.
    pub fn new() -> Self {
        let (notify_tx, _) = broadcast::channel::<()>(16);
        let handle = Self {
            inner: Arc::new(RwLock::new(HealthInner {
                config: MitigationConfig::default(),
                audit: AuditLog::default(),
                snapshot: HealthSnapshot::default(),
            })),
            notify_tx,
        };
        watchdog::spawn_watchdog(handle.clone());
        handle
    }

    pub async fn config(&self) -> MitigationConfig {
        self.inner.read().await.config.clone()
    }

    pub async fn set_config(&self, config: MitigationConfig) {
        self.inner.write().await.config = config;
        let _ = self.notify_tx.send(());
    }

    pub async fn toggle(&self, m: Mitigation, enabled: bool) {
        let mut inner = self.inner.write().await;
        inner.config.set_enabled(m, enabled);
        inner.audit.push(AuditEntry::now(
            m,
            if enabled { "enabled" } else { "disabled" },
            m.label(),
            true,
        ));
        drop(inner);
        let _ = self.notify_tx.send(());
    }

    pub async fn is_enabled(&self, m: Mitigation) -> bool {
        self.inner.read().await.config.is_enabled(m)
    }

    pub async fn push_audit(&self, entry: AuditEntry) {
        self.inner.write().await.audit.push(entry);
    }

    pub async fn audit_recent(&self, n: usize) -> Vec<AuditEntry> {
        self.inner.read().await.audit.recent(n).to_vec()
    }

    pub async fn snapshot(&self) -> HealthSnapshot {
        self.inner.read().await.snapshot.clone()
    }

    pub async fn set_snapshot(&self, snap: HealthSnapshot) {
        self.inner.write().await.snapshot = snap;
    }
}
