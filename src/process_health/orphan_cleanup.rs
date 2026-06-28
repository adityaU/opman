//! Orphaned process detection and cleanup.
//!
//! Scans /proc for processes whose command line matches known opencode/opman
//! child patterns but whose parent PID is 1 (reparented to init = orphan).
//! Optionally sends SIGTERM to clean them up.

use tracing::{debug, info, warn};

use super::{AuditEntry, Mitigation};

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

/// Check if the process is one of our own orphaned children.
///
/// Matches on the **executable** (argv[0] basename, plus the subcommand for
/// `opencode serve`) — NOT a substring anywhere in the full command line. This is
/// critical: opman launches `claude --bg` agents with `--settings`/`--mcp-config`
/// JSON that embeds the opman exe path (`…/opman claude-hook`, `…/opman mcp-ui`), so a
/// naive cmdline substring match on "opman" would flag those background agents as
/// orphans and SIGTERM them — opman killing its own running agents. claude agents have
/// argv[0] == "claude" and are explicitly never matched here.
fn matches_known_pattern(pid: u32) -> bool {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let raw = match std::fs::read(&cmdline_path) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let argv: Vec<String> = raw
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    argv_is_own_child(&argv)
}

/// Whether an argv belongs to one of our own (orphanable) processes. Pure helper for
/// [`matches_known_pattern`], split out for testing.
fn argv_is_own_child(argv: &[String]) -> bool {
    let Some(exe) = argv.first() else {
        return false;
    };
    let base = std::path::Path::new(exe)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| exe.clone());

    // Never touch claude background agents (opman embeds its own path in their args).
    if base == "claude" {
        return false;
    }

    let sub = argv.get(1).map(|s| s.as_str()).unwrap_or("");
    // Our own orphaned processes: any `opman …`, the `opencode-serve` wrapper, or
    // `opencode serve …`.
    base == "opman"
        || base == "opencode-serve"
        || (base == "opencode" && sub == "serve")
}

#[cfg(test)]
mod tests {
    use super::argv_is_own_child;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    // The regression: a claude background agent whose args embed the opman exe path
    // (via --settings / --mcp-config) must NOT be treated as our orphan.
    #[test]
    fn claude_bg_agent_with_opman_in_args_is_not_matched() {
        let a = argv(&[
            "claude",
            "--bg",
            "--resume",
            "uuid",
            "--settings",
            r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"/home/ubuntu/workspace/opman/target/release/opman claude-hook"}]}]}}"#,
            "--mcp-config",
            r#"{"mcpServers":{"ui":{"command":"/home/ubuntu/workspace/opman/target/release/opman","args":["mcp-ui"]}}}"#,
        ]);
        assert!(!argv_is_own_child(&a));
    }

    #[test]
    fn own_processes_are_matched() {
        assert!(argv_is_own_child(&argv(&["/home/ubuntu/workspace/opman/target/release/opman", "mcp-ui"])));
        assert!(argv_is_own_child(&argv(&["opman", "--web-only"])));
        assert!(argv_is_own_child(&argv(&["/usr/bin/opencode", "serve"])));
        assert!(argv_is_own_child(&argv(&["opencode-serve"])));
    }

    #[test]
    fn unrelated_processes_are_not_matched() {
        assert!(!argv_is_own_child(&argv(&["/usr/bin/opencode", "tui"]))); // not `serve`
        assert!(!argv_is_own_child(&argv(&["node", "/some/opman/script.js"])));
        assert!(!argv_is_own_child(&[]));
    }
}

/// Send SIGTERM to a process. Returns true on success.
fn send_sigterm(pid: u32) -> bool {
    // SAFETY: kill(2) is safe with a valid signal number.
    unsafe { libc::kill(pid as i32, libc::SIGTERM) == 0 }
}
