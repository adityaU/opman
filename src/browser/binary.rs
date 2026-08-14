//! Finding a browser to drive.
//!
//! Split from [`super::chrome`] because "which binary" and "how it is launched" fail in
//! completely different ways: the first is a host survey, the second a process dance.

use std::path::{Path, PathBuf};

/// Candidate binaries, in preference order. Playwright's bundled build is tried last:
/// it is the most likely to exist on a dev box but the most likely to be pruned.
const CANDIDATES: [&str; 5] = [
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "brave-browser",
];

/// First usable browser binary, or `None` if the host has none.
///
/// Snap-packaged builds are deliberately last. Snap confinement limits file access to
/// `~/snap/<name>`, so a snap Chromium cannot create the profile directory opman hands it
/// and aborts on startup — it is a working browser for a person and a broken one for
/// automation. Playwright's cached build, when present, is a plain unconfined binary and
/// is the better default here even though it is not on `PATH`.
pub fn find() -> Option<PathBuf> {
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
fn is_snap(path: &Path) -> bool {
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
pub fn which(name: &str) -> Option<PathBuf> {
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
#[path = "binary_tests.rs"]
mod binary_tests;
