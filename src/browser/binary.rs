//! Finding a browser to drive.
//!
//! Split from [`super::chrome`] because "which binary" and "how it is launched" fail in
//! completely different ways: the first is a host survey, the second a process dance.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub use super::install::install_guide;

/// Override the automatic browser search when Chromium is installed outside PATH.
pub const BROWSER_BIN_ENV: &str = "OPMAN_BROWSER_BIN";

#[derive(Clone, Debug, Serialize)]
pub struct BrowserInstallCommand {
    pub label: &'static str,
    pub command: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrowserInstallGuide {
    pub platform: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub steps: &'static [&'static str],
    pub commands: &'static [BrowserInstallCommand],
    pub env_var: &'static str,
    pub docs_url: &'static str,
    pub issue: BrowserSetupIssue,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserSetupIssue {
    NoBrowser,
    OverrideMissing { path: String },
    OverrideNotExecutable { path: String },
}

#[derive(Clone, Debug)]
pub enum BrowserUnavailable {
    NoBrowser,
    OverrideMissing(PathBuf),
    OverrideNotExecutable(PathBuf),
}

impl std::fmt::Display for BrowserUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBrowser => f.write_str("no Chromium-based browser was found"),
            Self::OverrideMissing(path) => write!(
                f,
                "OPMAN_BROWSER_BIN points to a missing path: {}",
                path.display()
            ),
            Self::OverrideNotExecutable(path) => write!(
                f,
                "OPMAN_BROWSER_BIN points to a non-executable file: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BrowserUnavailable {}

impl BrowserUnavailable {
    pub fn install_guide(&self) -> BrowserInstallGuide {
        let mut guide = install_guide();
        guide.issue = match self {
            Self::NoBrowser => BrowserSetupIssue::NoBrowser,
            Self::OverrideMissing(path) => BrowserSetupIssue::OverrideMissing {
                path: path.display().to_string(),
            },
            Self::OverrideNotExecutable(path) => BrowserSetupIssue::OverrideNotExecutable {
                path: path.display().to_string(),
            },
        };
        guide
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BrowserOverride {
    NotSet,
    Usable(PathBuf),
    Missing(PathBuf),
    NotExecutable(PathBuf),
}

/// Candidate binaries, in preference order. Playwright's bundled build is tried last:
/// it is the most likely to exist on a dev box but the most likely to be pruned.
const CANDIDATES: [&str; 5] = [
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "brave-browser",
];

/// Find the selected browser binary, refusing a bad explicit override.
///
/// Snap-packaged builds are deliberately last. Snap confinement limits file access to
/// `~/snap/<name>`, so a snap Chromium cannot create the profile directory opman hands it
/// and aborts on startup — it is a working browser for a person and a broken one for
/// automation. Playwright's cached build, when present, is a plain unconfined binary and
/// is the better default here even though it is not on `PATH`.
pub fn find() -> Result<PathBuf, BrowserUnavailable> {
    match browser_override() {
        BrowserOverride::Usable(path) => return Ok(path),
        BrowserOverride::Missing(path) => return Err(BrowserUnavailable::OverrideMissing(path)),
        BrowserOverride::NotExecutable(path) => {
            return Err(BrowserUnavailable::OverrideNotExecutable(path));
        }
        BrowserOverride::NotSet => {}
    }

    let on_path: Vec<PathBuf> = CANDIDATES.iter().filter_map(|name| which(name)).collect();

    on_path
        .iter()
        .find(|path| !is_snap(path))
        .cloned()
        .or_else(platform_browser)
        .or_else(playwright_chromium)
        .or_else(|| on_path.into_iter().next())
        .ok_or(BrowserUnavailable::NoBrowser)
}

/// Validate an explicit choice before looking at an already-running browser. A stale
/// process must not make a bad override appear to work by being adopted silently.
pub fn validate_override() -> Result<(), BrowserUnavailable> {
    match browser_override() {
        BrowserOverride::Missing(path) => Err(BrowserUnavailable::OverrideMissing(path)),
        BrowserOverride::NotExecutable(path) => {
            Err(BrowserUnavailable::OverrideNotExecutable(path))
        }
        BrowserOverride::NotSet | BrowserOverride::Usable(_) => Ok(()),
    }
}

fn browser_override() -> BrowserOverride {
    let Some(raw) = std::env::var_os(BROWSER_BIN_ENV) else {
        return BrowserOverride::NotSet;
    };
    let path = PathBuf::from(raw);
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(_) => return BrowserOverride::Missing(path),
    };
    if metadata.is_file() && is_executable(&metadata) {
        BrowserOverride::Usable(path)
    } else {
        BrowserOverride::NotExecutable(path)
    }
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
}

fn platform_browser() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file());
    }

    #[cfg(target_os = "windows")]
    {
        let roots = ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"];
        let suffixes = [
            "Google\\Chrome\\Application\\chrome.exe",
            "Chromium\\Application\\chrome.exe",
            "BraveSoftware\\Brave-Browser\\Application\\brave.exe",
        ];
        return roots
            .into_iter()
            .filter_map(std::env::var_os)
            .flat_map(|root| {
                suffixes
                    .iter()
                    .map(move |suffix| PathBuf::from(&root).join(suffix))
            })
            .find(|path| path.is_file());
    }

    None
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
