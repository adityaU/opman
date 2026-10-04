//! macOS launches GUI apps with a bare PATH (/usr/bin:/bin:...), so opman would not
//! find user-installed CLIs such as opencode, claude or a Homebrew git. Ask the
//! user's login shell for the PATH their terminal sees instead.

use std::ffi::OsString;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const FALLBACK_SHELL: &str = "/bin/zsh";
/// Shell startup files may print banners; the PATH is whatever follows this.
const MARKER: &str = "__OPMAN_PATH__";
const TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// The login shell's PATH on macOS; `None` elsewhere, or if the shell misbehaves,
/// in which case the child inherits this process's environment.
pub fn login_path() -> Option<OsString> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    match read_login_path() {
        Ok(path) => Some(path),
        Err(err) => {
            eprintln!("opman-desktop: using the inherited PATH: {err}");
            None
        }
    }
}

fn read_login_path() -> io::Result<OsString> {
    let shell = std::env::var_os("SHELL")
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| FALLBACK_SHELL.into());
    let mut child = Command::new(shell)
        .args(["-ilc", &format!("printf '{MARKER}%s' \"$PATH\"")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            // Best effort: the shell is abandoned either way.
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "login shell hung"));
        }
        thread::sleep(POLL_INTERVAL);
    };
    if !status.success() {
        return Err(io::Error::other(format!("login shell failed ({status})")));
    }

    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_string(&mut output)?;
    }
    let path = output
        .rsplit_once(MARKER)
        .map(|(_, path)| path.trim())
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::other("login shell printed no PATH"))?;
    Ok(path.into())
}
