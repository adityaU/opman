//! Keeping a pane's window off the person's screen.
//!
//! Every pane is its own browser window (a headed browser paints only the foreground tab
//! of a window, so panes cannot share one). On a virtual display that is invisible
//! anyway; on a real desktop it would be a stream of windows appearing under the person's
//! cursor. So in device mode each window is created at, and then moved to, a point far
//! outside every monitor. The screencast keeps painting it — the occlusion flags in
//! [`super::flags`] stop the browser from treating it as hidden.
//!
//! Two steps because platforms differ in which one they honour: `Target.createTarget`
//! takes a position on recent browsers, but window placement code may pull a new window
//! back onto a screen, while an explicit `Browser.setWindowBounds` afterwards is taken as
//! given on X11 and Windows. macOS is the exception: it constrains windows so part of the
//! title bar stays reachable, so there a parked window may sit at a screen edge rather than
//! vanish entirely.

use serde_json::{json, Value};

use super::cdp::Cdp;
use super::mode::{Placement, OFFSCREEN_ORIGIN};
use super::types::Viewport;

/// Parameters for `Target.createTarget` — a new window, sized to the pane, and parked when
/// the placement asks for it.
pub fn create_target_params(viewport: Viewport, placement: Placement) -> Value {
    let mut params = json!({
        "url": "about:blank",
        "newWindow": true,
        "width": viewport.width(),
        "height": viewport.height(),
    });
    if placement == Placement::OffScreen {
        if let Some(object) = params.as_object_mut() {
            object.insert("left".into(), OFFSCREEN_ORIGIN.into());
            object.insert("top".into(), OFFSCREEN_ORIGIN.into());
            // Do not raise the window, or the browser to the front with it.
            object.insert("background".into(), true.into());
        }
    }
    params
}

/// Parameters for `Browser.setWindowBounds` that park a window. `normal` is the only state
/// a position may be combined with; a maximised window would ignore it.
pub fn park_params(window_id: i64, viewport: Viewport) -> Value {
    json!({
        "windowId": window_id,
        "bounds": {
            "left": OFFSCREEN_ORIGIN,
            "top": OFFSCREEN_ORIGIN,
            "width": viewport.width(),
            "height": viewport.height(),
            "windowState": "normal",
        },
    })
}

/// Move a target's window off-screen. Best effort: a browser that cannot report or move
/// windows still renders the pane, it is just visible — not worth failing the open over.
pub async fn park(cdp: &Cdp, target_id: &str, viewport: Viewport) {
    let window = cdp
        .call(
            "Browser.getWindowForTarget",
            json!({ "targetId": target_id }),
        )
        .await;
    let window_id = match window {
        Ok(reply) => reply.get("windowId").and_then(Value::as_i64),
        Err(error) => {
            tracing::debug!(%error, "could not find the pane's window to park it");
            None
        }
    };
    let Some(window_id) = window_id else {
        return;
    };
    let moved = cdp
        .call("Browser.setWindowBounds", park_params(window_id, viewport))
        .await;
    if let Err(error) = moved {
        tracing::debug!(%error, "could not move the pane's window off-screen");
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod window_tests;
