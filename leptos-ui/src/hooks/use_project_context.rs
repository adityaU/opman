//! Project context — provides the active project index as a reactive signal.
//! Panels use this to save/restore per-project internal state on project switch.

use leptos::prelude::*;

/// Reactive signal for the active project index.
/// Provided as context by ChatLayout so panels can track project switches.
#[derive(Clone, Copy)]
pub struct ProjectContext {
    pub index: Memo<usize>,
}
