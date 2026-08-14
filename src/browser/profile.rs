//! What a Chromium profile directory says about who is using it.
//!
//! Chromium takes an exclusive lock on a profile: a second process pointed at the same
//! `--user-data-dir` prints "Failed to create a ProcessSingleton" and exits *without* a
//! DevTools banner. That is not hypothetical here — opman's browser is a child process,
//! and a child outlives a parent that was killed rather than shut down, so after a
//! restart the profile is routinely still held by the browser from the previous run.
//!
//! Reading the lock first turns that into the good outcome: a live holder that published
//! a DevTools endpoint is adopted (the user's tabs survive the restart), and a lock whose
//! process is gone is cleared instead of blocking every future launch.

use std::path::Path;

/// The three files Chromium creates to claim a profile. Removing a stale claim means
/// removing all three — leaving `SingletonSocket` behind makes the next launch fail with
/// a different message rather than succeed.
const CLAIM_FILES: [&str; 3] = ["SingletonLock", "SingletonCookie", "SingletonSocket"];

/// Where Chromium writes the port it picked for `--remote-debugging-port=0`: the port on
/// the first line, the browser target path on the second.
const PORT_FILE: &str = "DevToolsActivePort";

/// Who holds a profile directory.
#[derive(Debug, PartialEq, Eq)]
pub enum Owner {
    /// Nothing holds it; launch normally.
    Free,
    /// A claim left behind by a process that is gone. Safe to remove.
    Stale,
    /// A browser still has it, and where its DevTools socket is — `None` when it was
    /// launched without one, which is unusable to us but must not be evicted.
    Live { pid: u32, ws_url: Option<String> },
}

/// Inspect a profile directory without touching it.
pub fn owner(dir: &Path) -> Owner {
    let Some(pid) = lock_holder(dir) else {
        return Owner::Free;
    };
    let ws_url = devtools_endpoint(dir);
    if holder_is_running(pid, dir, ws_url.is_some()) {
        return Owner::Live { pid, ws_url };
    }
    Owner::Stale
}

/// Drop a claim whose owner is gone. Best effort: a file that is already missing, or that
/// a racing launch removed first, is exactly the state this wants.
pub fn release(dir: &Path) {
    for name in CLAIM_FILES {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

/// Whether a process is still around. Used for an adopted browser, which is not a child
/// of this process and so cannot be waited on.
#[cfg(target_os = "linux")]
pub fn pid_alive(pid: u32) -> bool {
    proc_entry(pid, "cmdline").is_some()
}

/// Without `/proc` there is nothing cheap to check, and claiming a browser we adopted has
/// died would relaunch into its still-held profile lock.
#[cfg(not(target_os = "linux"))]
pub fn pid_alive(_pid: u32) -> bool {
    true
}

/// The pid inside `SingletonLock`, whose target is `<hostname>-<pid>`. The hostname can
/// itself contain dashes, so the pid is what follows the *last* one.
fn lock_holder(dir: &Path) -> Option<u32> {
    let target = std::fs::read_link(dir.join(CLAIM_FILES[0])).ok()?;
    let target = target.to_str()?;
    target.rsplit_once('-')?.1.parse().ok()
}

/// The websocket Chromium is serving DevTools on, from the port file it writes on start
/// and deletes on a clean exit.
fn devtools_endpoint(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(dir.join(PORT_FILE)).ok()?;
    let mut lines = contents.lines();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    let path = lines.next().unwrap_or("").trim();
    if path.is_empty() {
        return None;
    }
    Some(format!("ws://127.0.0.1:{port}{path}"))
}

/// Whether the lock holder is really still running.
///
/// On Linux the pid is checked against `/proc`, and the command line must still name this
/// profile — pids are recycled, and evicting a live browser is worse than any delay.
#[cfg(target_os = "linux")]
fn holder_is_running(pid: u32, dir: &Path, _published_endpoint: bool) -> bool {
    let Some(cmdline) = proc_entry(pid, "cmdline") else {
        return false;
    };
    cmdline.contains(dir.to_string_lossy().as_ref())
}

/// Without `/proc`, the port file is the only evidence available: Chromium removes it on
/// a clean exit, so its presence alongside a lock means a browser is probably up.
#[cfg(not(target_os = "linux"))]
fn holder_is_running(_pid: u32, _dir: &Path, published_endpoint: bool) -> bool {
    published_endpoint
}

/// Read one `/proc/<pid>/<name>` entry, with NULs flattened to spaces so `cmdline` can be
/// searched as text. `None` means the process is gone (or `/proc` is not there at all).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn proc_entry(pid: u32, name: &str) -> Option<String> {
    let mut raw = std::fs::read(format!("/proc/{pid}/{name}")).ok()?;
    for byte in &mut raw {
        if *byte == 0 {
            *byte = b' ';
        }
    }
    Some(String::from_utf8_lossy(&raw).into_owned())
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod profile_tests;
