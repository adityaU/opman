//! Locating and launching the headless Chromium that backs every browser pane.
//!
//! One process serves the whole app: panes are separate *targets* on a single
//! DevTools websocket (see [`super::cdp`]), so opening a tenth pane costs a tab, not a
//! browser. The launch is deliberately minimal — no `--disable-web-security`, no
//! remote-allow-origins — because the socket is bound to loopback on an ephemeral port
//! that only this process ever learns.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// How long to wait for Chromium to print its websocket banner before giving up.
const BANNER_TIMEOUT: Duration = Duration::from_secs(20);

/// Candidate binaries, in preference order. Playwright's bundled build is tried last:
/// it is the most likely to exist on a dev box but the most likely to be pruned.
const CANDIDATES: [&str; 5] = [
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "brave-browser",
];

/// A launched browser process and the DevTools endpoint it is listening on.
pub struct Chrome {
    child: Child,
    ws_url: String,
}

impl Chrome {
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// Launch a headless Chromium and read back its websocket endpoint.
    ///
    /// Sandboxed first. Hosts that disable unprivileged user namespaces — Ubuntu 23.10+
    /// under AppArmor, and most containers — make Chromium's sandbox impossible to start,
    /// and it says so and exits. Retrying unsandboxed there is the difference between the
    /// feature working and not existing; doing it *only* there is what keeps the sandbox
    /// on every host that can honour it.
    pub async fn launch(user_data_dir: &std::path::Path) -> anyhow::Result<Self> {
        let binary = find_binary()
            .ok_or_else(|| anyhow::anyhow!("no Chromium or Chrome binary found on PATH"))?;

        let first = match Self::spawn(&binary, user_data_dir, Sandbox::Enabled).await {
            Ok(chrome) => return Ok(chrome),
            Err(error) => error,
        };

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
            Ok(Ok(ws_url)) => Ok(Self { child, ws_url }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(LaunchError::Timeout),
        }
    }

    /// Ask the process to exit. Dropping also kills it, but an explicit close lets the
    /// pool report failures instead of swallowing them in a destructor.
    pub async fn shutdown(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }

    /// Whether the process is still running. A crashed Chromium must be relaunched
    /// rather than reconnected, so the pool checks this before handing out a tab.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
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

    let mut lines = BufReader::new(stderr).lines();
    let mut sandbox_failed = false;
    while let Ok(Some(line)) = lines.next_line().await {
        if line.contains(NO_SANDBOX) {
            sandbox_failed = true;
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
    Err(if sandbox_failed {
        LaunchError::SandboxUnavailable
    } else {
        LaunchError::Exited
    })
}

/// First usable browser binary, or `None` if the host has none.
///
/// Snap-packaged builds are deliberately last. Snap confinement limits file access to
/// `~/snap/<name>`, so a snap Chromium cannot create the profile directory opman hands it
/// and aborts on startup — it is a working browser for a person and a broken one for
/// automation. Playwright's cached build, when present, is a plain unconfined binary and
/// is the better default here even though it is not on `PATH`.
fn find_binary() -> Option<PathBuf> {
    let on_path: Vec<PathBuf> = CANDIDATES.iter().filter_map(|name| which(name)).collect();

    on_path
        .iter()
        .find(|path| !is_snap(path))
        .cloned()
        .or_else(playwright_chromium)
        .or_else(|| on_path.into_iter().next())
}

/// Whether a binary is a snap, either by living under `/snap` or by being the shell shim
/// Ubuntu installs at `/usr/bin/chromium-browser` that execs one.
fn is_snap(path: &std::path::Path) -> bool {
    if path.starts_with("/snap") {
        return true;
    }
    // Only the head matters — the shim is a few lines, and a real ELF's first bytes will
    // not contain the marker.
    let mut head = [0_u8; 512];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(read) = std::io::Read::read(&mut file, &mut head) else {
        return false;
    };
    String::from_utf8_lossy(&head[..read]).contains("/snap/bin/")
}

/// Minimal `which`: scan `PATH` for an executable entry. Avoids shelling out.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(name);
        candidate.is_file().then_some(candidate)
    })
}

/// Playwright caches builds as `~/.cache/ms-playwright/chromium-<rev>/chrome-linux/chrome`.
/// Revisions sort lexically close enough to numerically for "pick the newest" to hold,
/// and any of them works, so an imperfect pick is still a working browser.
fn playwright_chromium() -> Option<PathBuf> {
    let root = dirs::cache_dir()?.join("ms-playwright");
    let mut builds: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("chromium-"))
        })
        .collect();
    builds.sort();
    builds
        .into_iter()
        .rev()
        .map(|dir| dir.join("chrome-linux").join("chrome"))
        .find(|exe| exe.is_file())
}

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod chrome_tests;
