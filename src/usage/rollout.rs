//! The Codex CLI's session logs (`$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl`)
//! record the rate limits the server reported on each turn. They are the offline
//! fallback when the live usage call fails.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::codex;
use super::types::{normalize_iso, unix_to_iso, window_from_minutes, Credits, Window};

/// How many of the newest logs to look through: an API-key session records no limits,
/// so the very newest log may have nothing to offer.
const MAX_FILES: usize = 5;
/// Only the end of a log is read; the latest event is there, and logs grow large.
const TAIL_BYTES: u64 = 512 * 1024;

#[derive(Debug, PartialEq)]
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub credits: Option<Credits>,
    pub recorded_at: String,
}

pub async fn latest(home: PathBuf) -> Option<Snapshot> {
    tokio::task::spawn_blocking(move || latest_blocking(&home))
        .await
        .ok()
        .flatten()
}

fn latest_blocking(home: &Path) -> Option<Snapshot> {
    newest_logs(&home.join("sessions"))
        .iter()
        .find_map(|path| last_snapshot(path))
}

/// The newest logs, newest first. The directory layout and file names both sort by
/// date, so name order is time order without touching file metadata.
fn newest_logs(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for year in sorted_desc(root) {
        for month in sorted_desc(&year) {
            for day in sorted_desc(&month) {
                let logs = sorted_desc(&day)
                    .into_iter()
                    .filter(|path| is_rollout(path));
                found.extend(logs);
                if found.len() >= MAX_FILES {
                    found.truncate(MAX_FILES);
                    return found;
                }
            }
        }
    }
    found
}

fn is_rollout(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
}

fn sorted_desc(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .collect();
    paths.sort_unstable_by(|a, b| b.cmp(a));
    paths
}

fn last_snapshot(path: &Path) -> Option<Snapshot> {
    let text = read_tail(path)?;
    text.lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .find_map(parse_line)
}

fn read_tail(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TAIL_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    // The cut may land mid-line or mid-character; a lossy decode keeps every whole line.
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn parse_line(line: &str) -> Option<Snapshot> {
    let event: Value = serde_json::from_str(line).ok()?;
    let limits = event.get("payload")?.get("rate_limits")?;
    let windows: Vec<Window> = ["primary", "secondary"]
        .iter()
        .filter_map(|key| logged_window(limits.get(*key)?))
        .collect();
    if windows.is_empty() {
        return None;
    }
    let recorded_at = event
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(normalize_iso)?;
    Some(Snapshot {
        windows,
        credits: codex::credits(limits.get("credits")),
        recorded_at,
    })
}

fn logged_window(window: &Value) -> Option<Window> {
    let used = window.get("used_percent")?.as_f64()?;
    let minutes = window
        .get("window_minutes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let reset = window
        .get("resets_at")
        .and_then(Value::as_i64)
        .and_then(unix_to_iso);
    let (id, label) = window_from_minutes(minutes);
    Some(Window::new(id, label, used).resets_at(reset))
}

#[cfg(test)]
#[path = "rollout_tests.rs"]
mod tests;
