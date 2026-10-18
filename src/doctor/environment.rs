//! opman itself: version, config folder, log file, and the latest log problems.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::report::Check;

/// How much of the end of the log is searched for problems.
const LOG_WINDOW: u64 = 256 * 1024;
const SHOWN_LINES: usize = 5;

pub(super) async fn checks() -> Vec<Check> {
    let mut checks = vec![version().await];
    checks.push(config_dir());
    match crate::logging::log_path() {
        Ok(path) => {
            checks.push(log_writable(&path));
            checks.push(recent_problems(&path));
        }
        Err(error) => {
            checks.push(Check::fail("log file path is not available").detail(format!("{error:#}")))
        }
    }
    checks
}

async fn version() -> Check {
    let title = format!("opman {}", env!("CARGO_PKG_VERSION"));
    match crate::self_update::newer_release().await {
        Ok(None) => Check::pass(format!("{title} (latest release)")),
        Ok(Some(newer)) => Check::warn(format!("{title}: {} is available", newer.tag()))
            .detail("Run `opman update` to install it."),
        Err(error) => Check::pass(title).detail(format!("Could not check for updates: {error:#}")),
    }
}

fn config_dir() -> Check {
    let Some(base) = dirs::config_dir() else {
        return Check::fail("no config directory on this system");
    };
    let dir = base.join("opman");
    match writable_dir(&dir) {
        Ok(()) => Check::pass(format!("config directory is writable: {}", dir.display())),
        Err(error) => Check::fail(format!(
            "config directory is not writable: {}",
            dir.display()
        ))
        .detail(error.to_string()),
    }
}

/// Create the folder if needed and write and remove a small file in it.
fn writable_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(format!(".doctor-{}", std::process::id()));
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(probe)
}

fn log_writable(path: &Path) -> Check {
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(_) => Check::pass(format!("log file is writable: {}", path.display())),
        Err(error) => Check::fail(format!("log file is not writable: {}", path.display()))
            .detail(error.to_string()),
    }
}

fn recent_problems(path: &Path) -> Check {
    let text = match tail_text(path) {
        Ok(text) => text,
        Err(error) => return Check::warn("could not read the log file").detail(error.to_string()),
    };
    let problems = last_problems(&text, SHOWN_LINES);
    if problems.is_empty() {
        return Check::pass("no recent errors or warnings in the log");
    }
    Check::warn(format!(
        "the log has recent errors or warnings (last {})",
        problems.len()
    ))
    .detail(problems.join("\n"))
}

fn tail_text(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(LOG_WINDOW)))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The last `count` ERROR or WARN lines, oldest first.
fn last_problems(text: &str, count: usize) -> Vec<&str> {
    let mut found: Vec<&str> = text
        .lines()
        .rev()
        .filter(|line| line.contains(" ERROR ") || line.contains(" WARN "))
        .take(count)
        .collect();
    found.reverse();
    found
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod environment_tests;
