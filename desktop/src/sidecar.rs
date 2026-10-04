//! The bundled `opman` binary, run headless as this app's web backend.

use std::fmt;
use std::fs::File;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::shell_path;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How long opman gets to stop its runners after an interrupt before it is killed.
#[cfg(unix)]
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);
/// Nothing is sent before the kill on Windows, so there is nothing to wait for.
#[cfg(not(unix))]
const SHUTDOWN_GRACE: Duration = Duration::ZERO;

/// A loopback port opman serves its web UI on.
#[derive(Clone, Copy)]
pub struct Port(u16);

impl Port {
    /// Asks the OS for a free port. Another process could take it before opman
    /// binds it, but opman then exits and the splash reports it.
    pub fn ephemeral() -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        Ok(Self(listener.local_addr()?.port()))
    }

    fn addr(self) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, self.0))
    }

    pub fn url(self) -> Result<url::Url, url::ParseError> {
        url::Url::parse(&format!("http://{}/", self.addr()))
    }
}

pub enum SidecarError {
    Port(io::Error),
    Log(io::Error),
    Spawn {
        binary: PathBuf,
        source: io::Error,
    },
    Exited(ExitStatus),
    Lost(io::Error),
    TimedOut(Duration),
    /// The app shut the sidecar down, or is about to.
    Stopped,
}

impl fmt::Display for SidecarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Port(err) => write!(f, "No free local port for opman: {err}"),
            Self::Log(err) => write!(f, "Cannot open the opman log file: {err}"),
            Self::Spawn { binary, source } => {
                write!(f, "Cannot launch {}: {source}", binary.display())
            }
            Self::Exited(status) => write!(f, "opman exited unexpectedly ({status})."),
            Self::Lost(err) => write!(f, "Lost track of the opman process: {err}"),
            Self::TimedOut(limit) => write!(
                f,
                "opman did not start serving within {} seconds.",
                limit.as_secs()
            ),
            Self::Stopped => f.write_str("opman was stopped."),
        }
    }
}

enum Slot {
    Idle,
    Running(Child),
    Closed,
}

/// Owns the opman child process. Managed as Tauri state.
pub struct Sidecar(Mutex<Slot>);

impl Default for Sidecar {
    fn default() -> Self {
        Self(Mutex::new(Slot::Idle))
    }
}

impl Sidecar {
    fn slot(&self) -> MutexGuard<'_, Slot> {
        // A panic while holding the lock leaves the slot itself intact.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn spawn(&self, port: Port, log_path: &Path) -> Result<(), SidecarError> {
        let binary = binary_path().map_err(|source| SidecarError::Spawn {
            binary: PathBuf::from("opman"),
            source,
        })?;
        let log = File::create(log_path).map_err(SidecarError::Log)?;
        let stderr = log.try_clone().map_err(SidecarError::Log)?;

        let mut command = Command::new(&binary);
        command
            .args([
                "--web-only",
                "--exit-on-stdin-eof",
                "--web-bind",
                "127.0.0.1",
            ])
            .arg("--web-port")
            .arg(port.0.to_string())
            // The child holds the write end; when this app dies for any reason the
            // pipe closes and opman exits instead of lingering.
            .stdin(Stdio::piped())
            .stdout(log)
            .stderr(stderr);
        if let Some(path) = shell_path::login_path() {
            command.env("PATH", path);
        }
        hide_console(&mut command);

        // Held across the spawn so a concurrent shutdown cannot miss the child.
        let mut slot = self.slot();
        if matches!(*slot, Slot::Closed) {
            return Err(SidecarError::Stopped);
        }
        let child = command
            .spawn()
            .map_err(|source| SidecarError::Spawn { binary, source })?;
        *slot = Slot::Running(child);
        Ok(())
    }

    /// Ok while opman is running.
    pub fn check(&self) -> Result<(), SidecarError> {
        let mut slot = self.slot();
        let Slot::Running(child) = &mut *slot else {
            return Err(SidecarError::Stopped);
        };
        match child.try_wait() {
            Ok(None) => Ok(()),
            Ok(Some(status)) => Err(SidecarError::Exited(status)),
            Err(err) => Err(SidecarError::Lost(err)),
        }
    }

    pub fn wait_until_ready(&self, port: Port, timeout: Duration) -> Result<(), SidecarError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.check()?;
            if TcpStream::connect_timeout(&port.addr(), POLL_INTERVAL).is_ok() {
                return Ok(());
            }
            thread::sleep(POLL_INTERVAL);
        }
        Err(SidecarError::TimedOut(timeout))
    }

    /// Stops opman and refuses any later spawn. Safe to call more than once.
    pub fn shutdown(&self) {
        let Slot::Running(mut child) = std::mem::replace(&mut *self.slot(), Slot::Closed) else {
            return;
        };
        interrupt(&child);
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(_) => break,
            }
        }
        if let Err(err) = child.kill() {
            eprintln!("opman-desktop: failed to kill opman: {err}");
        }
        if let Err(err) = child.wait() {
            eprintln!("opman-desktop: failed to reap opman: {err}");
        }
    }
}

/// The bundler installs the sidecar next to this executable, without its target triple.
fn binary_path() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.with_file_name(format!("opman{}", std::env::consts::EXE_SUFFIX)))
}

/// SIGINT is what opman's web-only mode waits for before stopping its runners.
#[cfg(unix)]
fn interrupt(child: &Child) {
    let sent = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Err(err) = sent {
        eprintln!("opman-desktop: failed to interrupt opman: {err}");
    }
}

/// Windows has no console signal to send a windowless child; it is killed outright.
#[cfg(not(unix))]
fn interrupt(_child: &Child) {}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}
