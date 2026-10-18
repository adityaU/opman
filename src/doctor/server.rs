//! Is an opman server running, and does its agent-manager socket answer?
//!
//! The socket is named `opman-agent-manager-<pid>.sock`. Connecting and hanging up again
//! changes nothing in the server and needs no login.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::report::{Check, Section};

const PREFIX: &str = "opman-agent-manager-";
const SUFFIX: &str = ".sock";
const CONNECT_LIMIT: Duration = Duration::from_secs(2);

/// The process id in a socket file name, if the name is one of ours.
pub(super) fn socket_pid(file_name: &str) -> Option<u32> {
    file_name
        .strip_prefix(PREFIX)?
        .strip_suffix(SUFFIX)?
        .parse()
        .ok()
}

/// Every agent-manager socket in the folders a server may have put it in.
fn sockets() -> Vec<(u32, PathBuf)> {
    let mut dirs = vec![std::env::temp_dir(), PathBuf::from("/tmp")];
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
        dirs.push(PathBuf::from(runtime));
    }
    dirs.sort();
    dirs.dedup();
    dirs.iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let pid = socket_pid(entry.file_name().to_str()?)?;
            Some((pid, entry.path()))
        })
        .collect()
}

fn process_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}

pub(super) async fn section() -> Section {
    let found = sockets();
    if found.is_empty() {
        let note = "Nothing to check. Start opman to test the live server.";
        return Section::new(
            "Running server",
            vec![Check::pass("no opman server is running").detail(note)],
        );
    }
    let mut checks = Vec::with_capacity(found.len());
    for (pid, path) in found {
        checks.push(socket_check(pid, &path).await);
    }
    Section::new("Running server", checks)
}

async fn socket_check(pid: u32, path: &Path) -> Check {
    if !process_alive(pid) {
        return Check::warn(format!(
            "stale agent-manager socket (process {pid} is gone)"
        ))
        .detail(format!("{} can be deleted.", path.display()));
    }
    match connect(path).await {
        Ok(()) => Check::pass(format!(
            "opman server (process {pid}) answers on its agent-manager socket"
        ))
        .detail(path.display().to_string()),
        Err(reason) => Check::fail(format!(
            "opman server (process {pid}) does not answer on its socket"
        ))
        .detail(format!("{}: {reason}", path.display())),
    }
}

#[cfg(unix)]
async fn connect(path: &Path) -> Result<(), String> {
    match tokio::time::timeout(CONNECT_LIMIT, tokio::net::UnixStream::connect(path)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!(
            "no answer within {} seconds",
            CONNECT_LIMIT.as_secs()
        )),
    }
}

#[cfg(not(unix))]
async fn connect(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod server_tests;
