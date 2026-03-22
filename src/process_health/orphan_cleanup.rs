//! Orphaned process detection and cleanup.
//!
//! Scans /proc for processes whose command line matches known opencode/opman
//! child patterns but whose parent PID is 1 (reparented to init = orphan).
//! Optionally sends SIGTERM to clean them up.

use tracing::{debug, info, warn};

use super::{AuditEntry, Mitigation};

/// Known command substrings that indicate an opman/opencode child process.
const KNOWN_PATTERNS: &[&str] = &[
    "opencode serve",
    "opencode-serve",
    "opman",
];

/// Scan for orphaned processes and clean them up.
///
/// Returns `(orphan_pids, audit_entries)`.
pub async fn scan_and_clean() -> (Vec<u32>, Vec<AuditEntry>) {
    let my_pid = std::process::id();
    let mut orphans = Vec::new();
    let mut entries = Vec::new();

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return (orphans, entries),
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only numeric directory names (PIDs)
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Skip self
        if pid == my_pid {
            continue;
        }

        // Check if parent is init (PID 1) — indicates orphan
        if !is_orphan(pid) {
            continue;
        }

        // Check if command matches known patterns
        if !matches_known_pattern(pid) {
            continue;
        }

        orphans.push(pid);
        debug!("found orphan process: PID {}", pid);

        // Attempt SIGTERM
        let result = send_sigterm(pid);
        let detail = format!("PID {} (orphaned opencode child)", pid);
        if result {
            info!("sent SIGTERM to orphan PID {}", pid);
            entries.push(AuditEntry::now(
                Mitigation::OrphanCleanup,
                "sigterm",
                &detail,
                true,
            ));
        } else {
            warn!("failed to send SIGTERM to orphan PID {}", pid);
            entries.push(AuditEntry::now(
                Mitigation::OrphanCleanup,
                "sigterm_failed",
                &detail,
                false,
            ));
        }
    }

    (orphans, entries)
}

/// Check if process with given PID has parent PID == 1.
fn is_orphan(pid: u32) -> bool {
    let status_path = format!("/proc/{}/status", pid);
    let content = match std::fs::read_to_string(&status_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            let ppid: u32 = rest.trim().parse().unwrap_or(0);
            return ppid == 1;
        }
    }
    false
}

/// Check if the process command line matches any known pattern.
fn matches_known_pattern(pid: u32) -> bool {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let raw = match std::fs::read(&cmdline_path) {
        Ok(b) => b,
        Err(_) => return false,
    };
    // cmdline uses \0 as separator — join with space for matching
    let cmdline = raw
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s))
        .collect::<Vec<_>>()
        .join(" ");

    KNOWN_PATTERNS
        .iter()
        .any(|pat| cmdline.contains(pat))
}

/// Send SIGTERM to a process. Returns true on success.
fn send_sigterm(pid: u32) -> bool {
    // SAFETY: kill(2) is safe with a valid signal number.
    unsafe { libc::kill(pid as i32, libc::SIGTERM) == 0 }
}
