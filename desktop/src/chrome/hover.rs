//! macOS and Windows: poll the global cursor and show the bar while it is in
//! the top strip. Polling is needed because the cursor sits over native
//! controls (the traffic lights) where the page receives no events.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow, WebviewWindowBuilder, Wry};

use super::bar::{step, Bar, Step};

const POLL: Duration = Duration::from_millis(50);
/// Height of the strip, from the top of the window, that reveals the bar.
const REVEAL_STRIP: f64 = 10.0;
/// Height of the shown bar; the cursor may roam inside it without hiding it.
const BAR_HEIGHT: f64 = if cfg!(target_os = "macos") {
    28.0
} else {
    32.0
};

const PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos"
} else {
    "windows"
};

/// Whether the bar is shown. Managed state, read again after every page load.
#[derive(Default)]
pub struct Shown(AtomicBool);

pub fn prepare(
    builder: WebviewWindowBuilder<'_, Wry, AppHandle>,
) -> WebviewWindowBuilder<'_, Wry, AppHandle> {
    builder
        // macOS keeps its (transparent, overlaid) title bar for the traffic lights.
        .decorations(cfg!(target_os = "macos"))
        .initialization_script(include_str!("overlay.js").replace("__PLATFORM__", PLATFORM))
        .on_page_load(|window, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            let shown = window.state::<Shown>().0.load(Ordering::Acquire);
            if let Err(err) = sync_overlay(&window, shown) {
                eprintln!("opman-desktop: cannot restore title bar: {err}");
            }
        })
}

pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    window.app_handle().manage(Shown::default());
    #[cfg(target_os = "macos")]
    super::macos::traffic_lights(window, false)?;
    let window = window.clone();
    thread::Builder::new()
        .name("opman-titlebar".into())
        .spawn(move || run(&window))
        .map(drop)
        .map_err(tauri::Error::Io)
}

fn run(window: &WebviewWindow) {
    let mut bar = Bar::Hidden;
    loop {
        thread::sleep(POLL);
        if window.is_fullscreen().unwrap_or(false) {
            continue;
        }
        let zone = match bar {
            Bar::Hidden => REVEAL_STRIP,
            Bar::Shown | Bar::Leaving(_) => BAR_HEIGHT,
        };
        let Ok(inside) = cursor_within(window, zone) else {
            continue;
        };
        bar = match step(bar, inside, Instant::now()) {
            Step::Stay(next) => next,
            Step::Show => apply(window, Bar::Shown, bar),
            Step::Hide => apply(window, Bar::Hidden, bar),
        };
    }
}

/// Moves to `next`, or stays at `current` when the window refuses.
fn apply(window: &WebviewWindow, next: Bar, current: Bar) -> Bar {
    let shown = matches!(next, Bar::Shown);
    let result = sync_overlay(window, shown);
    #[cfg(target_os = "macos")]
    let result = result.and_then(|()| super::macos::traffic_lights(window, shown));
    if let Err(err) = result {
        eprintln!("opman-desktop: cannot change title bar: {err}");
        return current;
    }
    window.state::<Shown>().0.store(shown, Ordering::Release);
    next
}

fn sync_overlay(window: &WebviewWindow, shown: bool) -> tauri::Result<()> {
    window.eval(format!("window.__opmanChrome?.set({shown})"))
}

/// True when the cursor is horizontally inside the window and within `zone`
/// logical pixels of its top edge.
fn cursor_within(window: &WebviewWindow, zone: f64) -> tauri::Result<bool> {
    let PhysicalPosition { x, y } = window.cursor_position()?;
    let origin = window.inner_position()?;
    let size = window.inner_size()?;
    let zone = zone * window.scale_factor()?;

    let left = f64::from(origin.x);
    let top = f64::from(origin.y);
    Ok(x >= left && x < left + f64::from(size.width) && y >= top && y < top + zone)
}
