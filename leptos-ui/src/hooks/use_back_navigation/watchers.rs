//! Reactive watchers and popstate handler for back-navigation layers.

use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::debug_overlay::dbg_log;
use crate::hooks::use_mobile_state::MobileState;
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_panel_state::PanelState;

use super::{history_back, push_back_entry, BackLayer, LayerStack, BACK_MODALS};

// ── Popstate handler ───────────────────────────────────────────────

pub fn install_popstate_handler(
    stack: &LayerStack,
    custom_callbacks: &Rc<RefCell<Vec<(String, Rc<dyn Fn()>)>>>,
    modal_state: ModalState,
    panels: PanelState,
    mobile: MobileState,
) {
    let stack = stack.clone();
    let custom_callbacks = custom_callbacks.clone();

    let handler =
        Closure::<dyn Fn(web_sys::PopStateEvent)>::new(move |e: web_sys::PopStateEvent| {
            let mut s = stack.borrow_mut();
            if s.is_empty() {
                return;
            }
            let Some(layer) = s.pop() else { return };
            dbg_log(&format!("[BACK] popstate closing layer: {:?}", layer));
            e.stop_immediate_propagation();

            match layer {
                BackLayer::Modal(name) => modal_state.close(name),
                BackLayer::Panel(name) => match name {
                    "Terminal" => panels.terminal.close(),
                    "Editor" => panels.editor.close(),
                    "Git" => panels.git.close(),
                    "Debug" => panels.debug.close(),
                    _ => panels.set_sidebar_open.set(false),
                },
                BackLayer::MobileSheet => {
                    mobile.set_active_panel.set(None);
                    mobile.set_input_hidden.set(false);
                    mobile.set_dock_collapsed.set(true);
                }
                BackLayer::MobileSidebar => mobile.close_sidebar(),
                BackLayer::Custom(ref id) => {
                    let cb = custom_callbacks
                        .borrow()
                        .iter()
                        .find(|(cid, _)| cid == id)
                        .map(|(_, cb)| cb.clone());
                    if let Some(cb) = cb {
                        cb();
                    }
                    custom_callbacks.borrow_mut().retain(|(cid, _)| cid != id);
                }
            }
        });

    let window = web_sys::window().unwrap();
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    let _ = window.add_event_listener_with_callback_and_add_event_listener_options(
        "popstate",
        handler.as_ref().unchecked_ref(),
        &opts,
    );
    handler.forget();
}

// ── Panel toggle watcher ───────────────────────────────────────────

/// Watch a boolean signal and push/pop a `BackLayer::Panel` on open/close.
pub fn watch_panel_toggle(
    stack: &LayerStack,
    open_signal: ReadSignal<bool>,
    panel_name: &'static str,
) {
    let stack = stack.clone();
    let layer = BackLayer::Panel(panel_name);
    Effect::new(move |prev: Option<bool>| {
        let open = open_signal.get();
        if let Some(was_open) = prev {
            if open && !was_open {
                dbg_log(&format!("[BACK] pushing panel layer: {}", panel_name));
                push_back_entry(&format!("panel:{}", panel_name));
                stack.borrow_mut().push(layer.clone());
            } else if !open && was_open {
                let mut s = stack.borrow_mut();
                if let Some(pos) = s.iter().rposition(|l| *l == layer) {
                    dbg_log(&format!("[BACK] {} closed, removing layer", panel_name));
                    s.remove(pos);
                    drop(s);
                    history_back();
                }
            }
        }
        open
    });
}

// ── Modal watcher ──────────────────────────────────────────────────

pub fn watch_modals(stack: &LayerStack, modal_state: ModalState) {
    let stack = stack.clone();
    Effect::new(move |prev_open: Option<Vec<ModalName>>| {
        let mut currently_open = Vec::new();
        for &name in BACK_MODALS {
            if modal_state.is_open_tracked(name) {
                currently_open.push(name);
            }
        }

        let prev = prev_open.unwrap_or_default();

        // Newly opened
        for &name in &currently_open {
            if !prev.contains(&name) {
                dbg_log(&format!("[BACK] pushing modal layer: {:?}", name));
                push_back_entry(&format!("modal:{:?}", name));
                stack.borrow_mut().push(BackLayer::Modal(name));
            }
        }

        // Newly closed
        for &name in &prev {
            if !currently_open.contains(&name) {
                let mut s = stack.borrow_mut();
                if let Some(pos) = s
                    .iter()
                    .rposition(|l| matches!(l, BackLayer::Modal(n) if *n == name))
                {
                    s.remove(pos);
                    drop(s);
                    history_back();
                }
            }
        }

        currently_open
    });
}

// ── Mobile watchers ────────────────────────────────────────────────

pub fn watch_mobile_sheets(stack: &LayerStack, mobile: MobileState) {
    let stack = stack.clone();
    Effect::new(
        move |prev: Option<Option<crate::hooks::use_mobile_state::MobilePanel>>| {
            let current = mobile.active_panel.get();
            if let Some(was) = prev {
                match (was, current) {
                    (None, Some(_)) => {
                        push_back_entry("mobile_sheet");
                        stack.borrow_mut().push(BackLayer::MobileSheet);
                    }
                    (Some(_), None) => {
                        let mut s = stack.borrow_mut();
                        if let Some(pos) =
                            s.iter().rposition(|l| matches!(l, BackLayer::MobileSheet))
                        {
                            s.remove(pos);
                            drop(s);
                            history_back();
                        }
                    }
                    _ => {}
                }
            }
            current
        },
    );
}

pub fn watch_mobile_sidebar(stack: &LayerStack, mobile: MobileState) {
    let stack = stack.clone();
    Effect::new(move |prev: Option<bool>| {
        let open = mobile.sidebar_open.get();
        if let Some(was_open) = prev {
            if open && !was_open {
                push_back_entry("mobile_sidebar");
                stack.borrow_mut().push(BackLayer::MobileSidebar);
            } else if !open && was_open {
                let mut s = stack.borrow_mut();
                if let Some(pos) = s
                    .iter()
                    .rposition(|l| matches!(l, BackLayer::MobileSidebar))
                {
                    s.remove(pos);
                    drop(s);
                    history_back();
                }
            }
        }
        open
    });
}
