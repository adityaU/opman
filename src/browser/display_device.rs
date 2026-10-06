//! Which display device mode draws on: the person's real one, never a virtual one.
//!
//! macOS and Windows always have a desktop, so the browser simply opens on it. On Linux
//! the session's X display is preferred even under Wayland (through XWayland), because an
//! X client can position its own windows and a Wayland client cannot — and parking pane
//! windows off-screen is the whole point. A pure Wayland session still works; its windows
//! just cannot be parked. With no display at all there is nothing "device" about the
//! host, so it falls back to server behaviour and says why.

use super::display::Display;
use super::mode::BrowserMode;

/// Launch flags for [`Display::Native`].
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const NATIVE_FLAGS: &[&str] = &["--ozone-platform=wayland"];
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub const NATIVE_FLAGS: &[&str] = &[];

/// What a Linux desktop session offers, reduced to the decision.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
enum Session {
    /// A live X display (possibly XWayland).
    X(Box<str>),
    /// Wayland only.
    Wayland,
    /// Nothing to draw on.
    None,
}

impl Display {
    /// The display a browser in this mode should use.
    pub async fn for_mode(mode: BrowserMode) -> Self {
        match mode {
            BrowserMode::Server => Self::ensure().await,
            BrowserMode::Device => Self::device().await,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    async fn device() -> Self {
        Self::Native
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    async fn device() -> Self {
        let session = classify(super::display::host_display(), wayland_present());
        match session {
            Session::X(name) => Self::Host(name),
            Session::Wayland => {
                tracing::warn!(
                    "device browser: Wayland session without XWayland; pane windows cannot be \
                     moved off-screen and will be visible on the desktop"
                );
                Self::Native
            }
            Session::None => {
                tracing::warn!(
                    "device browser: no X or Wayland display in this session, so there is no \
                     device screen to draw on; falling back to a virtual display"
                );
                Self::ensure().await
            }
        }
    }
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
fn classify(x_display: Option<Box<str>>, wayland: bool) -> Session {
    match (x_display, wayland) {
        (Some(name), _) => Session::X(name),
        (None, true) => Session::Wayland,
        (None, false) => Session::None,
    }
}

/// Whether `WAYLAND_DISPLAY` names a socket that exists. Like `DISPLAY`, the variable can
/// outlive its compositor.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn wayland_present() -> bool {
    let Some(name) = std::env::var_os("WAYLAND_DISPLAY") else {
        return false;
    };
    let path = std::path::PathBuf::from(&name);
    if path.is_absolute() {
        return path.exists();
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|dir| std::path::Path::new(&dir).join(&name))
        .is_some_and(|socket| socket.exists())
}

#[cfg(test)]
#[path = "display_device_tests.rs"]
mod display_device_tests;
