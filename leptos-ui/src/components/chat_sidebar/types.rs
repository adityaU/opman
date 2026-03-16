//! Shared types and helpers for chat sidebar.

use leptos::prelude::*;
use std::collections::HashSet;
use wasm_bindgen::JsCast;

pub const MAX_VISIBLE_SESSIONS: usize = 8;
pub const PINNED_KEY: &str = "opman-pinned-sessions";

// ── Context menu state ──────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ContextMenuState {
    pub session_id: String,
    pub session_title: String,
    pub x: i32,
    pub y: i32,
    #[allow(dead_code)]
    pub project_idx: usize,
}

// ── Delete confirmation state ───────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DeleteConfirm {
    pub session_id: String,
    pub session_title: String,
}

// ── Remove project confirmation state ──────────────────────────────

#[derive(Clone, Debug)]
pub struct RemoveProjectConfirm {
    pub project_idx: usize,
    pub project_name: String,
}

// ── Pinned sessions persistence ─────────────────────────────────────

pub fn load_pinned_sessions() -> HashSet<String> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return HashSet::new(),
    };
    let storage = match window.local_storage().ok().flatten() {
        Some(s) => s,
        None => return HashSet::new(),
    };
    let raw = match storage.get_item(PINNED_KEY).ok().flatten() {
        Some(r) => r,
        None => return HashSet::new(),
    };
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(v) => v.into_iter().collect(),
        Err(_) => HashSet::new(),
    }
}

pub fn save_pinned_sessions(pinned: &HashSet<String>) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let storage = match window.local_storage().ok().flatten() {
        Some(s) => s,
        None => return,
    };
    let vec: Vec<&String> = pinned.iter().collect();
    if let Ok(json) = serde_json::to_string(&vec) {
        let _ = storage.set_item(PINNED_KEY, &json);
    }
}

// ── Relative time formatting ────────────────────────────────────────

pub fn format_time(epoch_secs: f64) -> String {
    if epoch_secs <= 0.0 {
        return String::new();
    }
    let now_ms = js_sys::Date::now();
    let then_ms = epoch_secs * 1000.0;
    let diff_ms = now_ms - then_ms;
    let diff_min = (diff_ms / 60000.0).floor() as i64;
    if diff_min < 1 {
        return "now".to_string();
    }
    if diff_min < 60 {
        return format!("{}m ago", diff_min);
    }
    let diff_hrs = diff_min / 60;
    if diff_hrs < 24 {
        return format!("{}h ago", diff_hrs);
    }
    let diff_days = diff_hrs / 24;
    if diff_days < 7 {
        return format!("{}d ago", diff_days);
    }
    // Fallback: short date
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(then_ms));
    let month = d.get_month(); // 0-indexed
    let day = d.get_date();
    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let m = month_names.get(month as usize).unwrap_or(&"???");
    format!("{} {}", m, day)
}

// ── Context menu dismiss effect ─────────────────────────────────────

/// Registers document-level click + Escape listeners to dismiss the context menu.
pub fn setup_ctx_menu_dismiss(
    ctx_menu: ReadSignal<Option<ContextMenuState>>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
) {
    Effect::new(move |_| {
        if ctx_menu.get().is_none() {
            return;
        }
        let doc = web_sys::window().unwrap().document().unwrap();

        let click_handler = wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || {
            set_ctx_menu.set(None);
        });
        let key_handler = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
            move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "Escape" {
                    set_ctx_menu.set(None);
                }
            },
        );

        let _ =
            doc.add_event_listener_with_callback("click", click_handler.as_ref().unchecked_ref());
        let _ =
            doc.add_event_listener_with_callback("keydown", key_handler.as_ref().unchecked_ref());

        // Clone Function handles for cleanup before forgetting closures.
        // .forget() leaks the Closure's Wasm allocation but is required because
        // on_cleanup requires Send+Sync and Closure<dyn Fn> is neither.
        let click_ref = click_handler
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        let key_ref = key_handler
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        click_handler.forget();
        key_handler.forget();

        on_cleanup({
            let doc = doc.clone();
            move || {
                let _ = doc.remove_event_listener_with_callback("click", &click_ref);
                let _ = doc.remove_event_listener_with_callback("keydown", &key_ref);
            }
        });
    });
}
