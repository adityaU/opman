//! GUI apps start with a bare PATH (/usr/bin:/bin:...), so opman would not find
//! user-installed CLIs such as npx, opencode, claude or a Homebrew git. Build the PATH
//! the user's terminal sees instead: the login shell's own PATH, then the well-known
//! install locations of node version managers and package managers, so a slow, odd or
//! hanging shell profile still leaves the common tools reachable.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const FALLBACK_SHELL: &str = "/bin/zsh";
/// Shell startup files may print banners; the PATH is the line after this one.
const MARKER: &str = "__OPMAN_PATH__";
/// nvm + oh-my-zsh profiles routinely take several seconds on a cold start.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Directories under `$HOME` that hold user-installed CLIs, in priority order.
const HOME_BINS: [&str; 9] = [
    ".local/bin",
    ".volta/bin",
    ".bun/bin",
    ".cargo/bin",
    ".npm-global/bin",
    "Library/pnpm",
    ".asdf/shims",
    ".local/share/mise/shims",
    ".deno/bin",
];
/// System-wide package manager locations.
const SYSTEM_BINS: [&str; 4] = [
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/home/linuxbrew/.linuxbrew/bin",
];

/// The PATH to give opman on unix; `None` on Windows, where GUI apps inherit the full
/// user PATH already.
pub fn login_path() -> Option<OsString> {
    if !cfg!(unix) {
        return None;
    }
    let shell_program = std::env::var_os("SHELL")
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| FALLBACK_SHELL.into());
    let shell = match read_login_path(&shell_program) {
        Ok(path) => Some(path),
        Err(err) => {
            eprintln!("opman-desktop: login shell PATH unavailable: {err}");
            None
        }
    };
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let extras = known_bins(home.as_deref());
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(merge(shell.as_deref(), &extras, &inherited)).ok()
}

/// `npx` resolved against `path`, for opman's `OPMAN_ACP_NPX`: agents launched through
/// npx then work even if something later narrows the PATH.
pub fn find_in(path: &OsString, program: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Shell PATH first, then known install dirs that exist, then the inherited PATH; each
/// directory once, in first-seen order.
fn merge(shell: Option<&str>, extras: &[PathBuf], inherited: &OsString) -> Vec<PathBuf> {
    let shell_dirs = shell
        .into_iter()
        .flat_map(|path| std::env::split_paths(path));
    let mut merged: Vec<PathBuf> = Vec::new();
    for dir in shell_dirs
        .chain(extras.iter().cloned())
        .chain(std::env::split_paths(inherited))
    {
        if dir.as_os_str().is_empty() || merged.contains(&dir) {
            continue;
        }
        merged.push(dir);
    }
    merged
}

/// Install locations that exist on this machine, including the active nvm node.
fn known_bins(home: Option<&Path>) -> Vec<PathBuf> {
    let home_dirs = home.into_iter().flat_map(|home| {
        let nvm = nvm_bin(&home.join(".nvm"));
        HOME_BINS.iter().map(move |rel| home.join(rel)).chain(nvm)
    });
    home_dirs
        .chain(SYSTEM_BINS.iter().map(PathBuf::from))
        .filter(|dir| dir.is_dir())
        .collect()
}

/// The bin dir of nvm's default node: the `default` alias when it names an installed
/// version, otherwise the newest installed one.
fn nvm_bin(nvm: &Path) -> Option<PathBuf> {
    let versions = nvm.join("versions/node");
    let alias = std::fs::read_to_string(nvm.join("alias/default")).unwrap_or_default();
    let alias = alias.trim().trim_start_matches('v');
    let mut installed: Vec<String> = std::fs::read_dir(&versions)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    installed.sort_by_key(|name| version_key(name));
    let chosen = installed
        .iter()
        .rev()
        .find(|name| !alias.is_empty() && name.trim_start_matches('v').starts_with(alias))
        .or_else(|| installed.last())?;
    Some(versions.join(chosen).join("bin"))
}

/// `v20.11.1` → `[20, 11, 1]`, so versions sort numerically.
fn version_key(name: &str) -> Vec<u64> {
    name.trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

fn read_login_path(shell: &std::ffi::OsStr) -> io::Result<String> {
    // `printenv` prints the exported, colon-joined PATH in every shell (fish included,
    // where "$PATH" is a list). An interactive profile may exec into tmux, wait on a
    // prompt or leave a daemon holding stdout open, so the PATH is taken as soon as it
    // is printed and the whole run is time-boxed.
    let mut child = Command::new(shell)
        .args(["-ilc", &format!("echo {MARKER}; /usr/bin/printenv PATH")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("login shell has no stdout"))?;
    let (sender, receiver) = mpsc::channel();
    // Detached on timeout: a hung profile must not hang the app with it.
    thread::spawn(move || {
        let _ = sender.send(marked_line(BufReader::new(stdout)));
    });
    let received = receiver.recv_timeout(TIMEOUT);
    // Best effort either way: the shell has done its job or is being abandoned.
    let _ = child.kill();
    let _ = child.wait();
    match received {
        Ok(Some(path)) => Ok(path),
        Ok(None) => Err(io::Error::other("login shell printed no PATH")),
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "login shell hung")),
    }
}

/// The first non-empty line after the marker line, read as it arrives.
fn marked_line(reader: impl BufRead) -> Option<String> {
    let mut lines = reader.lines().map_while(Result::ok);
    lines.find(|line| line.trim() == MARKER)?;
    lines
        .map(|line| line.trim().to_string())
        .find(|line| !line.is_empty())
}

#[cfg(test)]
#[path = "shell_path_tests.rs"]
mod tests;
