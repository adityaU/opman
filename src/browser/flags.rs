//! The command line a browser is launched with, per mode.
//!
//! Split from [`super::chrome`] so the flag sets are plain data that can be tested
//! without a browser to launch.

use super::mode::BrowserMode;

/// Render at two device pixels per CSS pixel — server mode only.
///
/// A screencast frame is a copy of the compositor surface, and the surface is sized by
/// *this* number — not by the per-pane device scale factor, which only tells the page what
/// to believe. Left at one, a pane on a retina display is a 1x image stretched over 2x
/// pixels, which is exactly the softness it looked like. Panes that do not want the detail
/// are served a downscaled copy instead (see [`super::types::Viewport::capture_width`]),
/// so this costs bandwidth only where it buys sharpness.
///
/// Device mode leaves the scale to the real display: forcing it would change how the
/// person's own browser renders. Each tab measures the ratio it really got instead (see
/// [`super::tab::Tab::open`]), so a pane never asks for more than the surface holds.
///
/// Must agree with [`super::types::MAX_RATIO`]; a test holds them together.
pub const DEVICE_SCALE_FLAG: &str = "--force-device-scale-factor=2";

/// Where device mode's own windows open — any window the browser creates by itself, such
/// as one restored after a crash. Matches [`super::mode::OFFSCREEN_ORIGIN`].
pub const OFFSCREEN_POSITION_FLAG: &str = "--window-position=-16000,-16000";

/// Flags every launch gets.
const COMMON: &[&str] = &[
    // Port 0 = let the OS pick; the real port arrives on stderr.
    "--remote-debugging-port=0",
    "--no-first-run",
    "--no-default-browser-check",
    // Without these a window nobody can see — one no window manager ever raised, or one
    // parked off-screen — can be treated as hidden, and a throttled renderer paints no
    // screencast frames.
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
    // Drops the `navigator.webdriver` flag that bot checks read first.
    "--disable-blink-features=AutomationControlled",
    "--mute-audio",
    "--window-size=1280,800",
];

/// Software GL for a server with no GPU. Without it `WebGL` is simply absent, and a browser
/// with no WebGL context is one of the first things a bot check notices. The renderer
/// string will say SwiftShader; that is a smaller tell than having no renderer at all.
const SOFTWARE_GL: &[&str] = &[
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
];

const SERVER: &[&str] = &[
    "--disable-background-networking",
    "--disable-dev-shm-usage",
    "--window-position=0,0",
    DEVICE_SCALE_FLAG,
];

const DEVICE: &[&str] = &[
    // No window of its own: every window is a pane's, created (and parked) on demand. The
    // DevTools server keeps the browser alive with none open.
    "--no-startup-window",
    OFFSCREEN_POSITION_FLAG,
    // An off-screen window's timers would otherwise be clamped like a background tab's.
    "--disable-background-timer-throttling",
    // Windows' native occlusion tracker reports an off-screen window as hidden, which
    // stops it painting; the occluded-window flag above does not cover it.
    "--disable-features=CalculateNativeWinOcclusion",
    // After a hard stop the next launch would greet the person with "restore pages?".
    "--hide-crash-restore-bubble",
];

/// The mode-dependent launch flags, common ones first.
pub fn launch_flags(mode: BrowserMode) -> impl Iterator<Item = &'static str> {
    let specific = match mode {
        BrowserMode::Server => SERVER,
        BrowserMode::Device => DEVICE,
    };
    // Device mode has the person's real GPU; only a server needs the software fallback.
    let gl = match mode {
        BrowserMode::Server => SOFTWARE_GL,
        BrowserMode::Device => &[],
    };
    COMMON.iter().chain(specific).chain(gl).copied()
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod flags_tests;
