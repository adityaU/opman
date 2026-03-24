//! Listen for OS appearance changes and re-apply theme when appearance is "system".

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::types::api::ThemePair;

thread_local! {
    /// The latest theme pair (dark + light) stored for the system listener.
    static THEME_PAIR: RefCell<Option<ThemePair>> = RefCell::new(None);
    /// Whether the matchMedia listener has been installed.
    static LISTENER_INSTALLED: RefCell<bool> = RefCell::new(false);
}

/// Store the latest theme pair so the system listener can re-apply on OS change.
pub fn store_theme_pair(pair: &ThemePair) {
    THEME_PAIR.with(|cell| *cell.borrow_mut() = Some(pair.clone()));
}

/// Install the `matchMedia` change listener (idempotent — only installs once).
/// When the OS dark/light preference changes and the user has selected "system"
/// appearance, this will re-apply the correct theme variant.
pub fn install_system_listener() {
    LISTENER_INSTALLED.with(|cell| {
        if *cell.borrow() {
            return;
        }
        *cell.borrow_mut() = true;

        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(Some(mql)) = window.match_media("(prefers-color-scheme: dark)") else {
            return;
        };

        let cb = Closure::<dyn Fn()>::new(|| {
            on_system_change();
        });
        let _ = mql.add_event_listener_with_callback("change", cb.as_ref().unchecked_ref());
        cb.forget(); // leak — lives for the page lifetime
    });
}

/// Called when OS appearance changes. Re-applies theme if appearance is "system".
fn on_system_change() {
    let appearance = super::get_appearance();
    if appearance != "system" {
        return;
    }
    // Toggle light-theme class
    super::apply::set_appearance("system");

    // Re-apply the correct color variant
    THEME_PAIR.with(|cell| {
        let borrow = cell.borrow();
        let Some(pair) = borrow.as_ref() else { return };
        let effective = super::resolve_appearance("system");
        let colors = if effective == "light" {
            &pair.light
        } else {
            &pair.dark
        };
        super::apply_theme_to_css(colors);
    });
}
