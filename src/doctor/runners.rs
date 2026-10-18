//! The runner CLIs (`claude`, `codex`): do they start and report a version?

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use super::report::{Check, Section};
use super::tools::find_on_path;

const LIMIT: Duration = Duration::from_secs(5);
const RUNNERS: [&str; 2] = ["claude", "codex"];

pub(super) async fn section() -> Section {
    let path = std::env::var_os("PATH");
    let found = RUNNERS
        .iter()
        .filter_map(|name| find_on_path(name, path.as_deref()).map(|binary| (*name, binary)));
    let runs = found.map(|(name, binary)| async move { version(name, &binary, LIMIT).await });
    let mut checks = futures::future::join_all(runs).await;
    if checks.is_empty() {
        checks.push(
            Check::warn("no runner CLI (claude or codex) is installed").detail(
                "opman can still use other ACP agents, but the built-in runners need one of them.",
            ),
        );
    }
    Section::new("Runner CLIs", checks)
}

/// Run `<binary> --version` and report its first line, or the exact reason it did not work.
pub(super) async fn version(name: &str, binary: &Path, limit: Duration) -> Check {
    let mut command = Command::new(binary);
    command
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(limit, command.output()).await {
        Err(_) => {
            return Check::fail(format!(
                "{name} --version did not finish within {} seconds",
                limit.as_secs()
            ))
            .detail(binary.display().to_string());
        }
        Ok(Err(error)) => {
            return Check::fail(format!("{name} could not be started"))
                .detail(format!("{}: {error}", binary.display()));
        }
        Ok(Ok(output)) => output,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next().unwrap_or("").trim();
    if output.status.success() {
        let shown = if first.is_empty() {
            "(no version printed)"
        } else {
            first
        };
        return Check::pass(format!("{name} version: {shown}"));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Check::fail(format!("{name} --version failed ({})", output.status)).detail(stderr.trim())
}

#[cfg(test)]
#[path = "runners_tests.rs"]
mod runners_tests;
