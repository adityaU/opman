//! Background watchdog loop — periodic monitoring of FDs, memory, and connections.
//!
//! Runs on a configurable interval (default 10s). Skips checks for disabled
//! mitigations. Updates the shared `HealthSnapshot` and appends audit entries
//! when thresholds are breached.

use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, warn};

use super::{AuditEntry, HealthHandle, HealthSnapshot, Mitigation};

/// Default watchdog tick interval.
const TICK_INTERVAL: Duration = Duration::from_secs(10);

/// FD usage warning threshold (fraction of limit).
const FD_WARN_RATIO: f64 = 0.80;

/// Memory RSS warning threshold in bytes (2 GB).
const MEM_WARN_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// TCP connection count warning threshold.
const CONN_WARN_COUNT: u32 = 500;

/// Spawn the watchdog background task.
pub fn spawn_watchdog(handle: HealthHandle) {
    tokio::spawn(async move {
        run_loop(handle).await;
    });
}

async fn run_loop(handle: HealthHandle) {
    let mut ticker = interval(TICK_INTERVAL);
    let mut notify_rx = handle.notify_tx.subscribe();

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = notify_rx.recv() => {}
        }
        let config = handle.config().await;
        let mut snap = HealthSnapshot::default();

        // ── FD watchdog ─────────────────────────────────────────────
        if config.fd_watchdog {
            if let Some((used, limit)) = read_fd_counts() {
                snap.open_fds = Some(used);
                snap.fd_limit = Some(limit);
                if limit > 0 {
                    let ratio = used as f64 / limit as f64;
                    if ratio > FD_WARN_RATIO {
                        let msg = format!(
                            "FD usage high: {}/{} ({:.0}%)",
                            used,
                            limit,
                            ratio * 100.0
                        );
                        warn!("{}", msg);
                        handle
                            .push_audit(AuditEntry::now(
                                Mitigation::FdWatchdog,
                                "warning",
                                &msg,
                                true,
                            ))
                            .await;
                    }
                }
            }
        }

        // ── Memory watchdog ─────────────────────────────────────────
        if config.memory_watchdog {
            if let Some(rss) = read_self_rss() {
                snap.memory_rss_bytes = Some(rss);
                if rss > MEM_WARN_BYTES {
                    let msg = format!(
                        "RSS high: {} MB (threshold {} MB)",
                        rss / (1024 * 1024),
                        MEM_WARN_BYTES / (1024 * 1024)
                    );
                    warn!("{}", msg);
                    handle
                        .push_audit(AuditEntry::now(
                            Mitigation::MemoryWatchdog,
                            "warning",
                            &msg,
                            true,
                        ))
                        .await;
                }
            }
        }

        // ── Connection watchdog ─────────────────────────────────────
        if config.connection_watchdog {
            if let Some(count) = count_tcp_connections() {
                snap.tcp_connections = Some(count);
                if count > CONN_WARN_COUNT {
                    let msg = format!(
                        "TCP connections high: {} (threshold {})",
                        count, CONN_WARN_COUNT
                    );
                    warn!("{}", msg);
                    handle
                        .push_audit(AuditEntry::now(
                            Mitigation::ConnectionWatchdog,
                            "warning",
                            &msg,
                            true,
                        ))
                        .await;
                }
            }
        }

        // ── Orphan cleanup ──────────────────────────────────────────
        if config.orphan_cleanup {
            let (pids, entries) = super::orphan_cleanup::scan_and_clean().await;
            snap.orphan_pids = pids;
            for e in entries {
                handle.push_audit(e).await;
            }
        }

        // ── Port tracking ───────────────────────────────────────────
        if config.port_cleanup {
            let (ports, entries) = super::port_cleanup::scan_ports().await;
            snap.tracked_ports = ports;
            for e in entries {
                handle.push_audit(e).await;
            }
        }

        // ── Temp cleanup ────────────────────────────────────────────
        if config.temp_cleanup {
            let (files, entries) = super::temp_cleanup::scan_and_clean().await;
            snap.tracked_temp_files = files;
            for e in entries {
                handle.push_audit(e).await;
            }
        }

        handle.set_snapshot(snap).await;
        debug!("watchdog tick complete");
    }
}

// ── Linux-specific metric readers ───────────────────────────────────

/// Read open FD count and soft limit for the current process via /proc/self.
fn read_fd_counts() -> Option<(u64, u64)> {
    let fd_dir = std::fs::read_dir("/proc/self/fd").ok()?;
    let used = fd_dir.count() as u64;

    let limits = std::fs::read_to_string("/proc/self/limits").ok()?;
    let limit = parse_fd_limit(&limits)?;
    Some((used, limit))
}

fn parse_fd_limit(limits: &str) -> Option<u64> {
    for line in limits.lines() {
        if line.starts_with("Max open files") {
            // Format: "Max open files            1024                 1048576              files"
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Soft limit is at index 3
            if parts.len() >= 5 {
                return parts[3].parse().ok();
            }
        }
    }
    None
}

/// Read RSS of the current process from /proc/self/status.
fn read_self_rss() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let trimmed = rest.trim();
            let kb_str = trimmed.strip_suffix("kB").unwrap_or(trimmed).trim();
            let kb: u64 = kb_str.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// Count TCP connections from /proc/net/tcp + /proc/net/tcp6.
fn count_tcp_connections() -> Option<u32> {
    let mut count = 0u32;
    for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            // Skip header line
            count += content.lines().skip(1).count() as u32;
        }
    }
    Some(count)
}
