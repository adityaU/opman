//! The X display headed Chromium draws into.
//!
//! Browser panes used to run `--headless=new`, and a growing share of the web refuses to
//! serve it: the headless build ships a different user agent, misses codecs and
//! extensions, and trips every bot check that looks for them. A *headed* Chromium on a
//! virtual framebuffer is an ordinary desktop browser as far as the page can tell, and it
//! screencasts exactly the same way — so the pane keeps its pixels and loses the blocks.
//!
//! On a machine with a real session (`DISPLAY` set and its socket present) that display is
//! used as is. On a headless host — the normal case for opman — an `Xvfb` is started for
//! the browser and dies with it. If neither exists, the caller falls back to headless
//! rather than losing browser panes entirely.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

/// How long `Xvfb` gets to create its socket before it is declared broken.
const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_POLL: Duration = Duration::from_millis(50);

/// Display numbers to try. High numbers keep out of the way of any real session, and the
/// span leaves room for several opmans on one host.
const FIRST_DISPLAY: u8 = 99;
const LAST_DISPLAY: u8 = 119;

/// The framebuffer a virtual display advertises. Larger than any pane so a browser window
/// is never clipped by the screen it lives on.
const SCREEN: &str = "1920x1200x24";

/// Where Chromium should draw.
pub enum Display {
    /// A display that was already there. Not ours to stop.
    Host(Box<str>),
    /// An `Xvfb` started for this browser. Dropping the handle kills it.
    Virtual {
        name: Box<str>,
        _server: Child,
        // This follows the server so its files are removed after `kill_on_drop` runs.
        _cleanup: VirtualDisplayCleanup,
    },
    /// No X server and no way to start one: Chromium has to run headless after all.
    Headless,
}

impl Display {
    /// The best display available on this host.
    pub async fn ensure() -> Self {
        if let Some(name) = host_display() {
            tracing::debug!(%name, "using the host X display for browser panes");
            return Self::Host(name);
        }
        match Self::start_virtual().await {
            Ok(display) => display,
            Err(error) => {
                tracing::warn!(
                    "no X display and Xvfb could not start ({error}); browser panes will run \
                     headless, which some sites block"
                );
                Self::Headless
            }
        }
    }

    /// `DISPLAY` for the child, or `None` when there is nothing to point it at.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Host(name) => Some(name),
            Self::Virtual { name, .. } => Some(name),
            Self::Headless => None,
        }
    }

    /// The launch flags this display implies.
    ///
    /// `--ozone-platform=x11` rather than the default hint: what was found is an X socket,
    /// so a Chromium that decided to try Wayland first would have nowhere to draw.
    pub fn chrome_flags(&self) -> &'static [&'static str] {
        match self {
            Self::Headless => &["--headless=new", "--hide-scrollbars"],
            _ => &["--ozone-platform=x11"],
        }
    }

    async fn start_virtual() -> anyhow::Result<Self> {
        if which("Xvfb").is_none() {
            anyhow::bail!("Xvfb is not installed");
        }
        let mut last =
            anyhow::anyhow!("no free display number between :{FIRST_DISPLAY} and :{LAST_DISPLAY}");
        for number in FIRST_DISPLAY..=LAST_DISPLAY {
            match claim(number) {
                Claim::Taken => continue,
                // An X server killed before it could tidy up leaves its lock behind, and
                // Xvfb refuses a locked number. Left alone, a handful of crashes would
                // retire the whole range.
                Claim::Stale => {
                    tracing::info!(number, "clearing a stale X display lock");
                    let _ = std::fs::remove_file(lock(number));
                }
                Claim::Free => {}
            }
            match spawn_xvfb(number).await {
                Ok(display) => return Ok(display),
                // Losing a race for a number is expected when two opmans start together;
                // the next one along is as good.
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}

/// The session's own display, if it has one that is actually listening.
fn host_display() -> Option<Box<str>> {
    live_display(std::env::var("DISPLAY").ok()?.as_str())
}

/// A `DISPLAY` value, kept only if something is listening on it. The variable outlives the
/// server that set it often enough — a detached tmux, a closed login session — that the
/// socket has to be checked rather than trusted.
fn live_display(name: &str) -> Option<Box<str>> {
    let name = name.trim();
    let number: u8 = name
        .trim_start_matches(':')
        .split('.')
        .next()?
        .parse()
        .ok()?;
    listening(number).then(|| name.into())
}

async fn spawn_xvfb(number: u8) -> anyhow::Result<Display> {
    let name = format!(":{number}");
    let server = Command::new("Xvfb")
        .args([
            name.as_str(),
            "-screen",
            "0",
            SCREEN,
            // The browser talks over the unix socket; nothing off-host has any business
            // reaching this display.
            "-nolisten",
            "tcp",
            // Exit once the last client goes. `kill_on_drop` covers an orderly shutdown,
            // but a hard-killed opman cannot run destructors, and a display left behind
            // with no browser on it would burn a number from the range for good.
            "-terminate",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    let mut server = server;
    let deadline = std::time::Instant::now() + READY_TIMEOUT;
    loop {
        if listening(number) {
            tracing::info!(display = %name, "started a virtual X display for browser panes");
            return Ok(Display::Virtual {
                name: name.into_boxed_str(),
                _server: server,
                _cleanup: VirtualDisplayCleanup { number },
            });
        }
        if !matches!(server.try_wait(), Ok(None)) {
            anyhow::bail!("Xvfb {name} exited during startup");
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!(
                "Xvfb {name} was not listening within {}s",
                READY_TIMEOUT.as_secs()
            );
        }
        tokio::time::sleep(READY_POLL).await;
    }
}

/// Whether an X server is actually serving this display number.
///
/// A hard-killed X server — exactly what `kill_on_drop` produces — cannot unlink
/// `/tmp/.X11-unix/X{n}`, so the file can outlive the server. Trusting it would burn that
/// display number forever. `/tmp/.X11-unix` is also mode 0755 on some hosts (this one
/// included), so an unprivileged Xvfb cannot create the socket *file* and serves only on
/// Linux's abstract namespace. `/proc/net/unix` sees both kinds of binding without relying
/// on a connection attempt.
fn listening(number: u8) -> bool {
    let Ok(sockets) = std::fs::read_to_string("/proc/net/unix") else {
        return false;
    };
    socket_is_bound(&sockets, number)
}

/// Whether `/proc/net/unix` contains a binding for this display's pathname socket.
fn socket_is_bound(sockets: &str, number: u8) -> bool {
    let needle = format!("/tmp/.X11-unix/X{number}");
    sockets.lines().any(|line| {
        let Some(prefix) = line.strip_suffix(needle.as_str()) else {
            return false;
        };
        prefix.is_empty()
            || prefix.ends_with('@')
            || prefix
                .as_bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_whitespace())
    })
}

/// Whether a display number can be taken. The lock is a claim an X server writes before it
/// binds, and it holds the owner's pid — which is what tells a live claim from a corpse.
fn claim(number: u8) -> Claim {
    if listening(number) {
        return Claim::Taken;
    }
    verdict(std::fs::read_to_string(lock(number)).ok().as_deref())
}

/// What a lock file's contents mean. Split out from [`claim`] because the interesting part
/// — live pid, dead pid, no file — is decidable without an X server to test against.
fn verdict(lock_contents: Option<&str>) -> Claim {
    let Some(contents) = lock_contents else {
        return Claim::Free;
    };
    match contents.trim().parse::<u32>() {
        Ok(pid) if PathBuf::from(format!("/proc/{pid}")).exists() => Claim::Taken,
        _ => Claim::Stale,
    }
}

/// What a display number's lock says about it.
enum Claim {
    Free,
    /// Held by a live X server.
    Taken,
    /// Locked by a process that is gone.
    Stale,
}

fn socket(number: u8) -> PathBuf {
    PathBuf::from(format!("/tmp/.X11-unix/X{number}"))
}

fn lock(number: u8) -> PathBuf {
    PathBuf::from(format!("/tmp/.X{number}-lock"))
}

/// Removes files left by a hard-killed X server once its owned child has been killed.
pub(super) struct VirtualDisplayCleanup {
    number: u8,
}

impl Drop for VirtualDisplayCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(socket(self.number));
        let _ = std::fs::remove_file(lock(self.number));
    }
}

/// A `PATH` lookup, so a missing Xvfb is reported as such instead of as a spawn failure.
fn which(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod display_tests;
