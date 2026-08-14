//! Locating and launching the headless Chromium that backs every browser pane.
//!
//! One process serves the whole app: panes are separate *targets* on a single
//! DevTools websocket (see [`super::cdp`]), so opening a tenth pane costs a tab, not a
//! browser. The launch is deliberately minimal — no `--disable-web-security`, no
//! remote-allow-origins — because the socket is bound to loopback on an ephemeral port
//! that only this process ever learns.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use super::profile::{self, Owner};

/// How long to wait for Chromium to print its websocket banner before giving up.
const BANNER_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a browser already on the profile has to publish its DevTools port before it
/// is declared unusable — it takes the profile lock a moment before writing the file.
const ADOPT_GRACE: Duration = Duration::from_secs(5);
const ADOPT_POLL: Duration = Duration::from_millis(100);

/// A browser process and the DevTools endpoint it is listening on.
pub struct Chrome {
    process: Process,
    ws_url: String,
}

/// How this process relates to the browser. Adopting one it did not start is normal —
/// see [`super::profile`] — and the difference decides who may kill it.
enum Process {
    /// Launched here. Dropping the handle kills it.
    Owned(Child),
    /// Already running on the profile when we got here, so not ours to end: another
    /// opman may be driving it, and if nobody is, the next launch adopts it again.
    Adopted(u32),
}

impl Chrome {
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// Get a browser for a profile directory: the one already on it, or a new one.
    ///
    /// Sandboxed first. Hosts that disable unprivileged user namespaces — Ubuntu 23.10+
    /// under AppArmor, and most containers — make Chromium's sandbox impossible to start,
    /// and it says so and exits. Retrying unsandboxed there is the difference between the
    /// feature working and not existing; doing it *only* there is what keeps the sandbox
    /// on every host that can honour it.
    pub async fn launch(user_data_dir: &std::path::Path) -> anyhow::Result<Self> {
        if let Some(adopted) = Self::adopt(user_data_dir).await? {
            return Ok(adopted);
        }

        let binary = super::binary::find()
            .ok_or_else(|| anyhow::anyhow!("no Chromium or Chrome binary found on PATH"))?;

        let first = match Self::spawn(&binary, user_data_dir, Sandbox::Enabled).await {
            Ok(chrome) => return Ok(chrome),
            Err(error) => error,
        };

        // Whoever won the race owns the profile now; a second attempt only loses it again.
        if matches!(first, LaunchError::ProfileLocked) {
            return Err(first.into());
        }

        // A snap-confined Chromium dies without printing anything at all, so the retry
        // cannot be conditioned on recognising the sandbox message — only on the
        // sandboxed attempt having already failed.
        match first {
            LaunchError::SandboxUnavailable => tracing::warn!(
                "chromium's sandbox cannot start on this host (unprivileged user \
                 namespaces are disabled); browser panes will run unsandboxed"
            ),
            ref other => tracing::warn!(
                "sandboxed chromium failed to start ({}); retrying without the sandbox",
                anyhow::Error::from(other.clone())
            ),
        }
        Self::spawn(&binary, user_data_dir, Sandbox::Disabled)
            .await
            .map_err(Into::into)
    }

    /// Join the browser already on this profile, if there is one; clear its leftovers if
    /// there is not. `None` means the profile is ours to launch into.
    ///
    /// A browser found mid-startup holds the lock before it has written the port file, so
    /// a live holder without an endpoint is given [`ADOPT_GRACE`] to publish one.
    async fn adopt(user_data_dir: &std::path::Path) -> anyhow::Result<Option<Self>> {
        let deadline = std::time::Instant::now() + ADOPT_GRACE;
        loop {
            match profile::owner(user_data_dir) {
                Owner::Live {
                    pid,
                    ws_url: Some(ws_url),
                } => {
                    tracing::info!(pid, "adopting the chromium already holding the profile");
                    return Ok(Some(Self {
                        process: Process::Adopted(pid),
                        ws_url,
                    }));
                }
                Owner::Live { pid, ws_url: None } if std::time::Instant::now() >= deadline => {
                    return Err(anyhow::anyhow!(
                        "chromium (pid {pid}) is holding {} but is not serving DevTools; \
                         quit it and reopen the pane",
                        user_data_dir.display()
                    ))
                }
                Owner::Live { .. } => tokio::time::sleep(ADOPT_POLL).await,
                // The common case after a hard restart: the last browser is gone but its
                // claim on the profile is not, and Chromium refuses to start on a claimed
                // profile.
                Owner::Stale => {
                    tracing::info!("clearing a stale profile lock left by a previous browser");
                    profile::release(user_data_dir);
                    return Ok(None);
                }
                Owner::Free => return Ok(None),
            }
        }
    }

    async fn spawn(
        binary: &std::path::Path,
        user_data_dir: &std::path::Path,
        sandbox: Sandbox,
    ) -> Result<Self, LaunchError> {
        let mut child = Command::new(binary)
            .args([
                "--headless=new",
                // Port 0 = let the OS pick; the real port arrives on stderr.
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-networking",
                "--disable-backgrounding-occluded-windows",
                "--disable-renderer-backgrounding",
                "--disable-dev-shm-usage",
                "--hide-scrollbars",
                "--mute-audio",
                "--window-size=1280,800",
            ])
            .args(sandbox.flags())
            .arg(format!("--user-data-dir={}", user_data_dir.display()))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| LaunchError::Spawn(binary.display().to_string(), e.to_string()))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| LaunchError::Spawn(binary.display().to_string(), "no stderr".into()))?;

        match tokio::time::timeout(BANNER_TIMEOUT, read_ws_url(stderr)).await {
            Ok(Ok(ws_url)) => Ok(Self {
                process: Process::Owned(child),
                ws_url,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(LaunchError::Timeout),
        }
    }

    /// Ask the process to exit. Dropping also kills it, but an explicit close lets the
    /// pool report failures instead of swallowing them in a destructor. An adopted
    /// browser is left running: it was up before this process and its tabs outlive it.
    pub async fn shutdown(self) {
        let Process::Owned(mut child) = self.process else {
            return;
        };
        let _ = child.start_kill();
        let _ = child.wait().await;
    }

    /// Whether the process is still running. A crashed Chromium must be relaunched
    /// rather than reconnected, so the pool checks this before handing out a tab.
    pub fn is_alive(&mut self) -> bool {
        match &mut self.process {
            Process::Owned(child) => matches!(child.try_wait(), Ok(None)),
            Process::Adopted(pid) => profile::pid_alive(*pid),
        }
    }
}

/// Whether to ask for the OS sandbox. Not a bare `bool`, because the two call sites are
/// a security decision and `spawn(&binary, &dir, false)` says nothing at the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sandbox {
    Enabled,
    Disabled,
}

impl Sandbox {
    fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Enabled => &[],
            Self::Disabled => &["--no-sandbox", "--disable-setuid-sandbox"],
        }
    }
}

/// Why a launch did not produce a usable browser. `SandboxUnavailable` is separated out
/// because it is the one failure worth retrying differently.
#[derive(Clone, Debug)]
enum LaunchError {
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
                anyhow::anyhow!("chromium could not start its sandbox, and neither could it start without one")
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
async fn read_ws_url<R>(stderr: R) -> Result<String, LaunchError>
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
#[path = "chrome_tests.rs"]
mod chrome_tests;
