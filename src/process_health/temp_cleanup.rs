//! Temporary file tracking and cleanup.
//!
//! Scans known temp directories for files created by opman/opencode and
//! cleans up stale entries (older than the configured threshold).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

use super::{AuditEntry, Mitigation};

/// Max age for temp files before cleanup (1 hour).
const STALE_THRESHOLD: Duration = Duration::from_secs(3600);

/// Prefixes that indicate opman/opencode temp files.
const KNOWN_PREFIXES: &[&str] = &["opman-", "opencode-", ".opencode-", "oc-"];

/// Scan temp directories and clean stale files.
///
/// Returns `(remaining_temp_files, audit_entries)`.
pub async fn scan_and_clean() -> (Vec<String>, Vec<AuditEntry>) {
    let dirs = temp_dirs();
    let mut remaining = Vec::new();
    let mut entries = Vec::new();
    let now = SystemTime::now();

    for dir in &dirs {
        if !dir.is_dir() {
            continue;
        }
        let read_dir = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if !is_known_temp(&name_str) {
                continue;
            }

            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            // Check age
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => {
                    remaining.push(path_str);
                    continue;
                }
            };

            let modified = metadata.modified().unwrap_or(now);
            let age = now.duration_since(modified).unwrap_or_default();

            if age < STALE_THRESHOLD {
                // Still fresh — just track
                remaining.push(path_str);
                continue;
            }

            // Stale — attempt cleanup
            let result = remove_entry(&path);
            let detail = format!("{} (age: {}s)", path_str, age.as_secs());
            if result {
                info!("cleaned stale temp: {}", path_str);
                entries.push(AuditEntry::now(
                    Mitigation::TempCleanup,
                    "removed",
                    &detail,
                    true,
                ));
            } else {
                warn!("failed to clean temp: {}", path_str);
                remaining.push(path_str);
                entries.push(AuditEntry::now(
                    Mitigation::TempCleanup,
                    "remove_failed",
                    &detail,
                    false,
                ));
            }
        }
    }

    if !remaining.is_empty() {
        debug!("{} temp files tracked", remaining.len());
    }

    (remaining, entries)
}

/// Get list of temp directories to scan.
fn temp_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![std::env::temp_dir()];

    // Also check /tmp and /var/tmp if different from env temp
    for extra in &["/tmp", "/var/tmp"] {
        let p = PathBuf::from(extra);
        if p.is_dir() && !dirs.contains(&p) {
            dirs.push(p);
        }
    }

    // Check user-specific runtime dir (e.g. /run/user/1000)
    if let Some(runtime) = dirs::runtime_dir() {
        if runtime.is_dir() && !dirs.contains(&runtime) {
            dirs.push(runtime);
        }
    }

    dirs
}

/// Check if filename matches known temp file patterns.
fn is_known_temp(name: &str) -> bool {
    KNOWN_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// Remove a file or directory. Returns true on success.
fn remove_entry(path: &Path) -> bool {
    if path.is_dir() {
        std::fs::remove_dir_all(path).is_ok()
    } else {
        std::fs::remove_file(path).is_ok()
    }
}
