//! The browsers worth trying, per mode and per platform, in preference order.
//!
//! Pure data plus one expansion helper, so the orderings can be tested on any host even
//! though only one platform's list is ever consulted at runtime.

use std::path::PathBuf;

use super::mode::BrowserMode;

/// Names looked up on `PATH`.
///
/// Server mode wants an unbranded, automation-friendly Chromium first. Device mode wants
/// the browser the person actually uses — Chrome, then Edge, then Brave — and settles for
/// Chromium last.
pub fn path_names(mode: BrowserMode) -> &'static [&'static str] {
    match mode {
        BrowserMode::Server => &[
            "chromium",
            "chromium-browser",
            "google-chrome",
            "google-chrome-stable",
            "brave-browser",
            "microsoft-edge",
            "microsoft-edge-stable",
        ],
        BrowserMode::Device => &[
            "google-chrome-stable",
            "google-chrome",
            "microsoft-edge-stable",
            "microsoft-edge",
            "brave-browser",
            "brave",
            "chromium",
            "chromium-browser",
        ],
    }
}

/// macOS app bundles, relative to an `Applications` folder.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub fn mac_bundles(mode: BrowserMode) -> &'static [&'static str] {
    match mode {
        BrowserMode::Server => &[
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Chromium.app/Contents/MacOS/Chromium",
            "Brave Browser.app/Contents/MacOS/Brave Browser",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ],
        BrowserMode::Device => &[
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "Brave Browser.app/Contents/MacOS/Brave Browser",
            "Chromium.app/Contents/MacOS/Chromium",
        ],
    }
}

/// Windows installs, relative to `Program Files`, `Program Files (x86)` or
/// `%LOCALAPPDATA%` (per-user installs).
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
pub fn windows_suffixes(mode: BrowserMode) -> &'static [&'static str] {
    match mode {
        BrowserMode::Server => &[
            "Google\\Chrome\\Application\\chrome.exe",
            "Chromium\\Application\\chrome.exe",
            "BraveSoftware\\Brave-Browser\\Application\\brave.exe",
            "Microsoft\\Edge\\Application\\msedge.exe",
        ],
        BrowserMode::Device => &[
            "Google\\Chrome\\Application\\chrome.exe",
            "Microsoft\\Edge\\Application\\msedge.exe",
            "BraveSoftware\\Brave-Browser\\Application\\brave.exe",
            "Chromium\\Application\\chrome.exe",
        ],
    }
}

/// Every `root/relative` pair, browser-major: a preferred browser in any root beats a
/// less preferred one in the first root. (A per-user Chrome must not lose to a
/// machine-wide Chromium just because `Program Files` was searched first.)
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", test)),
    allow(dead_code)
)]
pub fn expand(roots: &[PathBuf], relative: &[&str]) -> Vec<PathBuf> {
    relative
        .iter()
        .flat_map(|suffix| roots.iter().map(move |root| root.join(suffix)))
        .collect()
}

#[cfg(test)]
#[path = "candidates_tests.rs"]
mod candidates_tests;
