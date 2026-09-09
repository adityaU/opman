//! Reading Chromium's startup chatter, which is the only channel it has to tell a parent
//! process how the launch went.
//!
//! There is no exit code to wait for and no status file: a browser that came up prints
//! `DevTools listening on ws://…` and keeps running, and one that did not prints why and
//! dies. Both arrive as stderr lines, so the difference between "retry unsandboxed" and
//! "tell the user their profile is held" is made here.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};

/// How long to wait for Chromium to print its websocket banner before giving up.
pub const BANNER_TIMEOUT: Duration = Duration::from_secs(20);

/// Why a launch did not produce a usable browser. `SandboxUnavailable` is separated out
/// because it is the one failure worth retrying differently.
#[derive(Clone, Debug)]
pub enum LaunchError {
    Spawn(String, String),
    SandboxUnavailable,
    ProfileLocked,
    Exited,
    Timeout,
}

impl From<LaunchError> for anyhow::Error {
    fn from(error: LaunchError) -> Self {
        match error {
            LaunchError::Spawn(binary, message) => {
                anyhow::anyhow!("failed to launch {binary}: {message}")
            }
            LaunchError::SandboxUnavailable => {
                anyhow::anyhow!(
                    "chromium could not start its sandbox, and neither could it start without one"
                )
            }
            LaunchError::ProfileLocked => anyhow::anyhow!(
                "another chromium claimed the browser profile while this one was starting"
            ),
            LaunchError::Exited => {
                anyhow::anyhow!("chromium exited before printing a DevTools endpoint")
            }
            LaunchError::Timeout => anyhow::anyhow!(
                "chromium did not report a DevTools endpoint within {}s",
                BANNER_TIMEOUT.as_secs()
            ),
        }
    }
}

/// Chromium announces `DevTools listening on ws://127.0.0.1:PORT/devtools/browser/UUID`
/// on stderr once the debugging socket is bound.
pub async fn read_ws_url<R>(stderr: R) -> Result<String, LaunchError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    const MARKER: &str = "DevTools listening on ";
    /// The fatal line a host with user namespaces disabled prints instead.
    const NO_SANDBOX: &str = "No usable sandbox";
    /// What Chromium says when the profile is already claimed. It aborts right after.
    const SINGLETON: &str = "ProcessSingleton";

    let mut lines = BufReader::new(stderr).lines();
    let mut sandbox_failed = false;
    let mut profile_locked = false;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.contains(NO_SANDBOX) {
            sandbox_failed = true;
            continue;
        }
        if line.contains(SINGLETON) {
            profile_locked = true;
            continue;
        }
        let Some(idx) = line.find(MARKER) else {
            continue;
        };
        let url = line[idx + MARKER.len()..].trim();
        if !url.is_empty() {
            return Ok(url.to_string());
        }
    }
    // Order matters: a locked profile aborts the launch before the sandbox is ever tried,
    // so it is the more specific explanation when both lines appeared.
    Err(match (profile_locked, sandbox_failed) {
        (true, _) => LaunchError::ProfileLocked,
        (false, true) => LaunchError::SandboxUnavailable,
        (false, false) => LaunchError::Exited,
    })
}

#[cfg(test)]
#[path = "banner_tests.rs"]
mod banner_tests;
