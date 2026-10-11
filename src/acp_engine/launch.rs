//! Starting an ACP agent process, and saying exactly why when it cannot start.
//!
//! "No such file or directory" from a spawn has three different causes, and only one of
//! them means the agent is missing: the working directory may not exist, or the command
//! may be a script (`npx` is `#!/usr/bin/env node`) whose interpreter is not on the PATH.
//! Version managers such as `n`, nvm or Volta install `node` beside `npx`, so the
//! command's own directory is put first on the child's PATH, which makes the script's
//! interpreter resolve wherever the script itself does.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tokio::process::{Child, Command};

use super::config::AgentConfig;

/// Why a spawn failed, worked out after the fact so the common case pays nothing.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LaunchFault {
    /// The project directory the agent should run in does not exist.
    MissingDir(PathBuf),
    /// Neither a path nor anything on the PATH names the command.
    MissingCommand,
    /// The command exists, but the interpreter on its `#!` line could not be found.
    MissingInterpreter(String),
    /// Anything else: permissions, resource limits, a bad binary.
    Other,
}

/// Spawn `agent` in `dir` with piped stdio.
pub(super) fn spawn(agent: &AgentConfig, dir: &str) -> Result<Child> {
    let mut cmd = Command::new(&agent.command);
    cmd.args(&agent.args);
    for key in agent.env_removals() {
        cmd.env_remove(key);
    }
    if let Some(path) = path_with_command_dir(&agent.command, std::env::var_os("PATH")) {
        cmd.env("PATH", path);
    }
    // Explicit config wins, PATH included.
    for (key, value) in &agent.env {
        cmd.env(key, value);
    }
    cmd.current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        // The agent's own logs are its business; keeping them off opman's stdout avoids
        // interleaving them with the TUI.
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    cmd.spawn().map_err(|error| {
        let fault = diagnose(&agent.command, Path::new(dir), &error);
        anyhow!(describe(&agent.command, &fault, &error))
    })
}

/// The PATH with the command's directory in front, when the command is a path.
fn path_with_command_dir(command: &str, path: Option<OsString>) -> Option<OsString> {
    let dir = Path::new(command)
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())?;
    let rest = path.unwrap_or_default();
    let dirs = std::iter::once(dir.to_path_buf())
        .chain(std::env::split_paths(&rest).filter(|entry| entry != dir));
    std::env::join_paths(dirs).ok()
}

pub(super) fn diagnose(command: &str, dir: &Path, error: &io::Error) -> LaunchFault {
    if !dir.is_dir() {
        return LaunchFault::MissingDir(dir.to_path_buf());
    }
    if error.kind() != io::ErrorKind::NotFound {
        return LaunchFault::Other;
    }
    let Some(resolved) = resolve(command) else {
        return LaunchFault::MissingCommand;
    };
    match interpreter(&resolved) {
        Some(name) => LaunchFault::MissingInterpreter(name),
        None => LaunchFault::Other,
    }
}

fn describe(command: &str, fault: &LaunchFault, error: &io::Error) -> String {
    match fault {
        LaunchFault::MissingDir(dir) => format!(
            "cannot start ACP agent `{command}`: the project folder {} does not exist",
            dir.display()
        ),
        LaunchFault::MissingCommand => {
            format!("failed to spawn ACP agent `{command}`: not found on PATH (is it installed?)")
        }
        LaunchFault::MissingInterpreter(name) => format!(
            "failed to spawn ACP agent `{command}`: it is a script that runs `{name}`, and \
             `{name}` is not on opman's PATH"
        ),
        LaunchFault::Other => format!("failed to spawn ACP agent `{command}`: {error}"),
    }
}

/// The command as a file: itself when it is a path, else its first PATH match.
fn resolve(command: &str) -> Option<PathBuf> {
    let as_path = Path::new(command);
    if as_path.components().count() > 1 {
        return as_path.is_file().then(|| as_path.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(command))
        .find(|candidate| candidate.is_file())
}

/// The program a script's `#!` line runs: `node` for `#!/usr/bin/env node`, `sh` for
/// `#!/bin/sh`. `None` for binaries.
fn interpreter(file: &Path) -> Option<String> {
    use std::io::Read;
    let mut head = [0u8; 256];
    let read = std::fs::File::open(file).ok()?.read(&mut head).ok()?;
    let line = head[..read].strip_prefix(b"#!")?;
    let line = String::from_utf8_lossy(line);
    let mut words = line.lines().next()?.split_whitespace();
    let program = words.next()?;
    let name = match Path::new(program).file_name()?.to_str()? {
        "env" => words.find(|word| !word.starts_with('-'))?,
        other => other,
    };
    Some(name.to_string())
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod launch_tests;
