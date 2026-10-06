//! Locating and launching the Chromium that backs every browser pane.
//!
//! One process serves the whole app: panes are separate *targets* on a single
//! DevTools websocket (see [`super::cdp`]), so opening a tenth pane costs a tab, not a
//! browser. The launch is deliberately minimal — no `--disable-web-security`, no
//! remote-allow-origins — because the socket is bound to loopback on an ephemeral port
//! that only this process ever learns.
//!
//! The browser is a real headed one, drawing into whatever display [`super::display`]
//! finds or starts. Headless Chromium is only the last resort: it announces itself in the
//! user agent and fails enough bot checks that many sites simply refuse it.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use super::banner::{read_ws_url, LaunchError, BANNER_TIMEOUT};
use super::display::Display;
use super::flags::launch_flags;
use super::mode::BrowserMode;
use super::profile::{self, Owner};

/// How long a browser already on the profile has to publish its DevTools port before it
/// is declared unusable — it takes the profile lock a moment before writing the file.
const ADOPT_GRACE: Duration = Duration::from_secs(5);
const ADOPT_POLL: Duration = Duration::from_millis(100);

/// How long a browser being replaced gets to shut down cleanly before its claim on the
/// profile is cleared out from under it.
const RETIRE_GRACE: Duration = Duration::from_secs(5);

/// A browser process and the DevTools endpoint it is listening on.
pub struct Chrome {
    process: Process,
    ws_url: String,
}

/// How this process relates to the browser. Adopting one it did not start is normal —
/// see [`super::profile`] — and the difference decides who may kill it.
enum Process {
    /// Launched here. Dropping the handle kills it — along with the virtual display it
    /// was drawing into, which is held here for exactly that reason and never read.
    /// Boxed to keep the variant near the size of `Adopted`; there is one of these per
    /// process, so the allocation is paid once.
    Owned(Box<Owned>),
    /// Already running on the profile when we got here, so not ours to end: another
    /// opman may be driving it, and if nobody is, the next launch adopts it again.
    Adopted(u32),
    /// Adopted by its DevTools port because the platform left no pid to read (Windows).
    /// Alive for as long as the port answers.
    Served(u16),
}

/// A browser launched here, with the virtual display it draws into.
struct Owned {
    child: Child,
    _display: Display,
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
    pub async fn launch(
        user_data_dir: &std::path::Path,
        mode: BrowserMode,
    ) -> anyhow::Result<Self> {
        // Validate this before adoption: an existing browser must not hide a bad path the
        // user explicitly configured.
        super::binary::validate_override().map_err(anyhow::Error::new)?;
        if let Some(adopted) = Self::adopt(user_data_dir).await? {
            return Ok(adopted);
        }

        let binary = super::binary::find(mode).map_err(anyhow::Error::new)?;
        tracing::info!(binary = %binary.display(), ?mode, "launching the pane browser");

        let display = Display::for_mode(mode).await;
        let launch = Launch {
            binary: &binary,
            user_data_dir,
            mode,
        };
        let (first, display) = match launch.spawn(display, Sandbox::Enabled).await {
            Ok(chrome) => return Ok(chrome),
            Err((error, display)) => (error, display),
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
        launch
            .spawn(display, Sandbox::Disabled)
            .await
            .map_err(|(error, _)| error.into())
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
                    // A headless holder is an upgrade leftover, and adopting it would make
                    // the switch to headed silently do nothing.
                    if profile::is_headless(pid) {
                        Self::retire(pid, user_data_dir).await;
                        return Ok(None);
                    }
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
                Owner::Serving { port, ws_url } => {
                    tracing::info!(port, "adopting the browser serving DevTools on the profile");
                    return Ok(Some(Self {
                        process: Process::Served(port),
                        ws_url,
                    }));
                }
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

    /// End a browser that cannot be adopted and clear its claim, so the caller can launch
    /// a usable one on the same profile.
    ///
    /// `SIGTERM` rather than a kill: Chromium flushes cookies and history on the way out,
    /// and those are the profile the next browser inherits.
    async fn retire(pid: u32, user_data_dir: &std::path::Path) {
        tracing::info!(pid, "replacing the headless chromium holding the profile");
        let terminated = Command::new("kill")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        if let Err(error) = terminated {
            tracing::warn!(pid, %error, "could not signal the headless browser");
        }

        let deadline = std::time::Instant::now() + RETIRE_GRACE;
        while profile::pid_alive(pid) && std::time::Instant::now() < deadline {
            tokio::time::sleep(ADOPT_POLL).await;
        }
        // Chromium normally removes its own claim; it is still here if the exit was not
        // clean, and a leftover claim blocks every launch that follows.
        profile::release(user_data_dir);
    }

    /// Ask the process to exit. Dropping also kills it, but an explicit close lets the
    /// pool report failures instead of swallowing them in a destructor. An adopted
    /// browser is left running: it was up before this process and its tabs outlive it.
    pub async fn shutdown(self) {
        let Process::Owned(mut owned) = self.process else {
            return;
        };
        let child = &mut owned.child;
        let _ = child.start_kill();
        let _ = child.wait().await;
    }

    /// Whether the process is still running. A crashed Chromium must be relaunched
    /// rather than reconnected, so the pool checks this before handing out a tab.
    pub fn is_alive(&mut self) -> bool {
        match &mut self.process {
            Process::Owned(owned) => matches!(owned.child.try_wait(), Ok(None)),
            Process::Adopted(pid) => profile::pid_alive(*pid),
            Process::Served(port) => profile::endpoint_answers(*port),
        }
    }
}

/// One launch attempt's fixed inputs; the sandbox and display vary between attempts.
struct Launch<'a> {
    binary: &'a std::path::Path,
    user_data_dir: &'a std::path::Path,
    mode: BrowserMode,
}

impl Launch<'_> {
    /// Start a browser on this display. The display comes back with any error so the
    /// unsandboxed retry reuses it instead of leaving one `Xvfb` behind per attempt.
    async fn spawn(
        &self,
        display: Display,
        sandbox: Sandbox,
    ) -> Result<Chrome, (LaunchError, Display)> {
        let binary = self.binary;
        let mut command = Command::new(binary);
        command
            .args(launch_flags(self.mode))
            .args(display.chrome_flags())
            .args(sandbox.flags())
            .arg(format!("--user-data-dir={}", self.user_data_dir.display()))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        display.apply_env(&mut command);

        let fail = |error: LaunchError, display| (error, display);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                let error = LaunchError::Spawn(binary.display().to_string(), e.to_string());
                return Err(fail(error, display));
            }
        };

        let Some(stderr) = child.stderr.take() else {
            let error = LaunchError::Spawn(binary.display().to_string(), "no stderr".into());
            return Err(fail(error, display));
        };

        match tokio::time::timeout(BANNER_TIMEOUT, read_ws_url(stderr)).await {
            Ok(Ok(ws_url)) => Ok(Chrome {
                process: Process::Owned(Box::new(Owned {
                    child,
                    _display: display,
                })),
                ws_url,
            }),
            Ok(Err(e)) => Err(fail(e, display)),
            Err(_) => Err(fail(LaunchError::Timeout, display)),
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

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod chrome_tests;
