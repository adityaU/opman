//! Shared types, constants, and helpers for the message timeline module.

use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Shared accordion state — survives component re-creation across reactive re-renders.
/// Key = tool-call ID or subagent session ID, Value = user-toggled expanded state.
/// Provided via context by MessageTimeline, consumed by ToolCallView / SubagentSession.
#[derive(Clone, Copy)]
pub struct AccordionState(pub RwSignal<HashMap<String, bool>>);

/// React: VIRTUALIZE_THRESHOLD = 40 groups before switching to virtual list.
pub const VIRTUALIZE_THRESHOLD: usize = 40;

/// React: estimated size per group in pixels (used for initial layout before measurement).
pub const ESTIMATED_ROW_HEIGHT: f64 = 160.0;

/// React: overscan — extra items to render above/below the viewport.
pub const OVERSCAN: usize = 5;

/// Schedule a closure on the next animation frame.
pub fn request_animation_frame(f: impl FnOnce() + 'static) {
    let cb = Closure::once_into_js(f);
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(cb.unchecked_ref());
    }
}
