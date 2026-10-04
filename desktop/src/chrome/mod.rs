//! Auto-hiding title bar. The page always fills the whole window; the bar floats
//! over it and only shows while the cursor is in the top strip.
//!
//! - macOS: transparent overlay title bar whose native traffic lights are hidden
//!   and shown ([`macos`]), plus a drag strip from the overlay script.
//! - Windows: native caption buttons cannot float over a webview without unsafe
//!   Win32 code, so the overlay script draws Windows 11 caption buttons that call
//!   the real minimize / maximize / close.
//! - Linux: a native GTK header bar floats over the webview ([`linux`]).

mod bar;
#[cfg(not(target_os = "linux"))]
mod hover;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(target_os = "linux"))]
use hover as platform;
#[cfg(target_os = "linux")]
use linux as platform;

use tauri::{AppHandle, WebviewWindow, WebviewWindowBuilder};

/// Builds a window from its config entry with the title bar wired in.
pub fn build_window(app: &AppHandle, label: &str) -> tauri::Result<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == label)
        .ok_or(tauri::Error::WindowNotFound)?;
    let window = platform::prepare(WebviewWindowBuilder::from_config(app, config)?).build()?;
    platform::install(&window)?;
    Ok(window)
}
