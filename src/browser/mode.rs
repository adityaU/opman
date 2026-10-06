//! Which browser backs the panes: one opman runs on a server, or the person's own.
//!
//! **Server** is the original design — a private Chromium, headed on a virtual display,
//! that nobody ever looks at directly. **Device** is for the desktop app: opman is running
//! on the person's own machine, so the browser behind the panes is the one they already
//! have installed (Chrome, Edge, Brave, Chromium — in that order), headed on their real
//! display with its windows parked off-screen. A real browser on a real display with a
//! persistent profile is what gets past the bot checks a fresh server Chromium fails.

use serde::Serialize;

/// Where the panes' browser comes from. Chosen once, at startup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserMode {
    /// A private Chromium on a virtual display. The right choice on a headless host.
    #[default]
    Server,
    /// The device's installed browser, headed on the real display, windows off-screen.
    Device,
}

/// Where a pane's window should sit on the screen it is drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Wherever the browser puts it. On a virtual display nobody can see it anyway.
    Default,
    /// Far outside every monitor, so the person's desktop never shows a window an agent
    /// is driving. The pane still sees it through the screencast.
    OffScreen,
}

/// The top-left corner a parked window is moved to. Well outside any real desktop, and
/// inside the signed 16-bit range X11 stores window positions in.
pub const OFFSCREEN_ORIGIN: i32 = -16_000;

impl BrowserMode {
    /// The mode the CLI asked for.
    pub fn from_device_flag(device_browser: DeviceBrowser) -> Self {
        match device_browser {
            DeviceBrowser::Requested => Self::Device,
            DeviceBrowser::NotRequested => Self::Server,
        }
    }

    /// The profile directory name under `<data_dir>/opman`.
    ///
    /// Separate per mode: Chrome 136+ refuses remote debugging on its *default* profile,
    /// so device mode needs a dedicated one anyway, and keeping the two apart means
    /// switching mode never hands a server Chromium the device browser's cookies.
    pub fn profile_dir_name(self) -> &'static str {
        match self {
            Self::Server => "browser-profile",
            Self::Device => "device-browser-profile",
        }
    }

    /// Where windows of a browser this mode launched belong.
    pub fn placement(self) -> Placement {
        match self {
            Self::Server => Placement::Default,
            Self::Device => Placement::OffScreen,
        }
    }
}

/// Whether `--device-browser` was given. A named type rather than a `bool` so the call
/// site that turns a CLI flag into a mode reads as what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceBrowser {
    Requested,
    NotRequested,
}

impl From<bool> for DeviceBrowser {
    fn from(requested: bool) -> Self {
        if requested {
            Self::Requested
        } else {
            Self::NotRequested
        }
    }
}

#[cfg(test)]
#[path = "mode_tests.rs"]
mod mode_tests;
