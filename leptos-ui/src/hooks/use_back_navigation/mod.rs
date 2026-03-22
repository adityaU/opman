//! Back-navigation hook — browser/system back button support for the whole app.
//!
//! Pushes browser history entries for transient UI states (modals, panels,
//! mobile sheets, mobile sidebar, mobile editor file-open). When the user
//! presses back (popstate), the hook closes the most recent transient layer
//! instead of navigating away.
//!
//! Coexists with `use_url_restore`'s popstate handler by using `history.state`
//! to tag transient entries with `{ "opman_back": true, "layer": "<id>" }`.

mod watchers;

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::components::debug_overlay::dbg_log;
use crate::hooks::use_mobile_state::MobileState;
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_panel_state::PanelState;

// ── Types ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(super) enum BackLayer {
    Modal(ModalName),
    Panel(&'static str),
    MobileSheet,
    MobileSidebar,
    Custom(String),
}

pub(super) type LayerStack = Rc<RefCell<Vec<BackLayer>>>;

/// All ModalName variants that get back-nav (heavy modals only).
pub(super) const BACK_MODALS: &[ModalName] = &[
    ModalName::Settings,
    ModalName::ContextWindow,
    ModalName::DiffReview,
    ModalName::CrossSearch,
    ModalName::SplitView,
    ModalName::SessionGraph,
    ModalName::SessionDashboard,
    ModalName::ActivityFeed,
    ModalName::AssistantCenter,
    ModalName::Inbox,
    ModalName::Memory,
    ModalName::Autonomy,
    ModalName::Routines,
    ModalName::Delegation,
    ModalName::Missions,
    ModalName::WorkspaceManager,
    ModalName::AddProject,
    ModalName::SystemMonitor,
    ModalName::SessionSearch,
    ModalName::Watcher,
    ModalName::Cheatsheet,
    ModalName::TodoPanel,
    ModalName::NotificationPrefs,
];

// ── History helpers ────────────────────────────────────────────────

fn make_back_state(layer_id: &str) -> JsValue {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"opman_back".into(), &JsValue::TRUE);
    let _ = js_sys::Reflect::set(&obj, &"layer".into(), &layer_id.into());
    obj.into()
}

/// Check whether a popstate event's state has `opman_back` set.
pub fn is_back_nav_state(state: &JsValue) -> bool {
    if state.is_null() || state.is_undefined() {
        return false;
    }
    js_sys::Reflect::get(state, &"opman_back".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub(super) fn push_back_entry(layer_id: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(history) = window.history().ok() else {
        return;
    };
    let state = make_back_state(layer_id);
    let url = window.location().href().unwrap_or_else(|_| "/".to_string());
    let _ = history.push_state_with_url(&state, "", Some(&url));
}

pub(super) fn history_back() {
    if let Some(w) = web_sys::window() {
        let _ = w.history().ok().map(|h| h.back());
    }
}

// ── BackNavigation context ─────────────────────────────────────────

/// Back-navigation state handle, provided via context.
#[derive(Clone)]
pub struct BackNavigation {
    stack: SendWrapper<LayerStack>,
    custom_callbacks: SendWrapper<Rc<RefCell<Vec<(String, Rc<dyn Fn()>)>>>>,
}

impl BackNavigation {
    #[allow(dead_code)]
    pub fn has_layers(&self) -> bool {
        !self.stack.borrow().is_empty()
    }

    /// Push a custom named layer with a back-dismiss callback.
    pub fn push_custom_layer(&self, id: &str, on_back: Rc<dyn Fn()>) -> String {
        let lid = format!("custom:{}", id);
        dbg_log(&format!("[BACK] pushing custom layer: {}", id));
        push_back_entry(&lid);
        self.stack
            .borrow_mut()
            .push(BackLayer::Custom(id.to_string()));
        self.custom_callbacks
            .borrow_mut()
            .push((id.to_string(), on_back));
        id.to_string()
    }

    /// Remove a custom layer (closed by UI, not by back button).
    pub fn remove_custom_layer(&self, id: &str) {
        let mut s = self.stack.borrow_mut();
        let pos = s
            .iter()
            .rposition(|l| matches!(l, BackLayer::Custom(ref cid) if cid == id));
        let Some(pos) = pos else { return };
        s.remove(pos);
        drop(s);
        self.custom_callbacks
            .borrow_mut()
            .retain(|(cid, _)| cid != id);
        history_back();
    }
}

// ── Hook ───────────────────────────────────────────────────────────

/// Install back-navigation support. Call once in `ChatLayout`.
pub fn use_back_navigation(
    modal_state: ModalState,
    panels: PanelState,
    mobile: MobileState,
) -> BackNavigation {
    let stack: LayerStack = Rc::new(RefCell::new(Vec::new()));
    let custom_callbacks: Rc<RefCell<Vec<(String, Rc<dyn Fn()>)>>> =
        Rc::new(RefCell::new(Vec::new()));

    watchers::install_popstate_handler(&stack, &custom_callbacks, modal_state, panels, mobile);
    watchers::watch_modals(&stack, modal_state);
    watchers::watch_panel_toggle(&stack, panels.terminal.open, "Terminal");
    watchers::watch_panel_toggle(&stack, panels.editor.open, "Editor");
    watchers::watch_panel_toggle(&stack, panels.git.open, "Git");
    watchers::watch_panel_toggle(&stack, panels.debug.open, "Debug");
    watchers::watch_mobile_sheets(&stack, mobile);
    watchers::watch_mobile_sidebar(&stack, mobile);

    let back_nav = BackNavigation {
        stack: SendWrapper::new(stack),
        custom_callbacks: SendWrapper::new(custom_callbacks),
    };
    provide_context(back_nav.clone());
    back_nav
}
