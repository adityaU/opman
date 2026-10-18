//! Starting the shell, and killing a command together with everything it started.

use std::path::Path;
use tokio::process::Command;

/// The user's shell when it understands `-c` like a POSIX shell, else `/bin/sh`.
#[cfg(unix)]
fn shell_path() -> String {
    const POSIX_LIKE: [&str; 5] = ["bash", "zsh", "sh", "dash", "ksh"];
    std::env::var("SHELL")
        .ok()
        .filter(|shell| {
            let path = Path::new(shell);
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| POSIX_LIKE.contains(&n))
        })
        .unwrap_or_else(|| "/bin/sh".into())
}

/// A shell command with piped output, no stdin, its own process group, and no pager.
pub fn shell_command(script: &str) -> Command {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new(shell_path());
        command.arg("-c").arg(script).process_group(0);
        command
    };
    #[cfg(not(unix))]
    let mut command = {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(script);
        command
    };
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("PAGER", "cat")
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true);
    command
}

/// Kills the command's whole process group when dropped, unless disarmed. Dropping is what
/// makes a cancelled tool call clean up after itself.
pub struct GroupKill(Option<u32>);

impl GroupKill {
    pub fn new(pid: Option<u32>) -> Self {
        Self(pid)
    }

    /// The command finished by itself: leave any background children alone.
    pub fn disarm(&mut self) {
        self.0 = None;
    }

    pub fn kill(&mut self) {
        if let Some(pid) = self.0.take() {
            kill_group(pid);
        }
    }
}

impl Drop for GroupKill {
    fn drop(&mut self) {
        self.kill();
    }
}

/// A negative pid signals the group. Use the executable, since POSIX shell builtins
/// disagree about accepting `--` before a negative operand.
#[cfg(unix)]
fn kill_group(pid: u32) {
    let _ = std::process::Command::new("/bin/kill")
        .args(["-KILL", "--"])
        .arg(format!("-{pid}"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}
