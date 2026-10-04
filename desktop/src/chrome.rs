//! Auto-hiding title bar.
//!
//! The window starts without decorations. While the cursor is in the top strip
//! of the window, the platform's own title bar (with its native minimize,
//! maximize and close buttons) is switched on; shortly after the cursor leaves,
//! it is switched off again. On macOS the bar is transparent.
//!
//! Hover is read by polling the global cursor position: once the native bar is
//! shown the cursor sits over non-client area, where the page gets no events.

use std::thread;
use std::time::{Duration, Instant};

use tauri::{PhysicalPosition, WebviewWindow};

const POLL: Duration = Duration::from_millis(50);
/// Height of the strip, from the top of the content, that reveals the bar.
const REVEAL_STRIP: f64 = 10.0;
/// How long the cursor may be outside the bar before it hides again.
const HIDE_DELAY: Duration = Duration::from_millis(400);

#[derive(Clone, Copy)]
enum Bar {
    Hidden,
    Shown,
    /// Shown, but the cursor left at this instant.
    Leaving(Instant),
}

/// Runs for the life of the window on its own thread.
pub fn spawn(window: WebviewWindow) -> std::io::Result<()> {
    thread::Builder::new()
        .name("opman-titlebar".into())
        .spawn(move || run(&window))
        .map(drop)
}

fn run(window: &WebviewWindow) {
    let mut bar = Bar::Hidden;
    loop {
        thread::sleep(POLL);
        // Fullscreen has no title bar to manage.
        if window.is_fullscreen().unwrap_or(false) {
            continue;
        }
        let Ok(inside) = in_reveal_zone(window) else {
            // No global cursor position (e.g. Wayland): keep the native bar so
            // the window can still be moved and closed.
            if let Err(err) = show(window) {
                eprintln!("opman-desktop: cannot show title bar: {err}");
            }
            return;
        };
        bar = match step(bar, inside, Instant::now()) {
            Step::Stay(next) => next,
            Step::Show => apply(window, show, Bar::Shown, bar),
            Step::Hide => apply(window, hide, Bar::Hidden, bar),
        };
    }
}

enum Step {
    Stay(Bar),
    Show,
    Hide,
}

fn step(bar: Bar, inside: bool, now: Instant) -> Step {
    match (bar, inside) {
        (Bar::Hidden, true) => Step::Show,
        (Bar::Hidden, false) => Step::Stay(Bar::Hidden),
        (Bar::Shown | Bar::Leaving(_), true) => Step::Stay(Bar::Shown),
        (Bar::Shown, false) => Step::Stay(Bar::Leaving(now)),
        (Bar::Leaving(since), false) if now.duration_since(since) >= HIDE_DELAY => Step::Hide,
        (Bar::Leaving(since), false) => Step::Stay(Bar::Leaving(since)),
    }
}

fn apply(
    window: &WebviewWindow,
    action: fn(&WebviewWindow) -> tauri::Result<()>,
    next: Bar,
    current: Bar,
) -> Bar {
    match action(window) {
        Ok(()) => next,
        Err(err) => {
            eprintln!("opman-desktop: cannot change title bar: {err}");
            current
        }
    }
}

/// True when the cursor is over the native bar (if shown) or the top strip of
/// the content, horizontally within the window.
fn in_reveal_zone(window: &WebviewWindow) -> tauri::Result<bool> {
    let PhysicalPosition { x, y } = window.cursor_position()?;
    let outer = window.outer_position()?;
    let inner = window.inner_position()?;
    let size = window.outer_size()?;
    let strip = REVEAL_STRIP * window.scale_factor()?;

    let left = f64::from(outer.x);
    let top = f64::from(outer.y);
    let bottom = f64::from(inner.y) + strip;
    Ok(x >= left && x < left + f64::from(size.width) && y >= top && y < bottom)
}

fn show(window: &WebviewWindow) -> tauri::Result<()> {
    window.set_decorations(true)?;
    // Turning decorations back on resets the macOS style mask.
    #[cfg(target_os = "macos")]
    window.set_title_bar_style(tauri::TitleBarStyle::Transparent)?;
    Ok(())
}

fn hide(window: &WebviewWindow) -> tauri::Result<()> {
    window.set_decorations(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_only_after_the_delay() {
        let start = Instant::now();
        let Step::Stay(leaving) = step(Bar::Shown, false, start) else {
            panic!("leaving must not hide at once");
        };
        assert!(matches!(
            step(leaving, false, start + HIDE_DELAY / 2),
            Step::Stay(Bar::Leaving(_))
        ));
        assert!(matches!(
            step(leaving, false, start + HIDE_DELAY),
            Step::Hide
        ));
    }

    #[test]
    fn returning_cancels_the_hide() {
        let start = Instant::now();
        assert!(matches!(
            step(Bar::Leaving(start), true, start),
            Step::Stay(Bar::Shown)
        ));
        assert!(matches!(step(Bar::Hidden, true, start), Step::Show));
    }
}
