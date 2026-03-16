//! URL state parsing and writing helpers for the URL restore hook.

use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::SseState;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

/// Parsed URL state from query params.
#[derive(Debug, Clone, Default)]
pub struct UrlState {
    pub session: Option<String>,
    pub project: Option<usize>,
    pub sidebar: Option<bool>,
    pub terminal: Option<bool>,
    pub editor: Option<bool>,
    pub git: Option<bool>,
}

/// Parse the current URL query params into a `UrlState`.
pub fn parse_url_state() -> UrlState {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return UrlState::default(),
    };
    let search = window.location().search().unwrap_or_default();
    let params = match web_sys::UrlSearchParams::new_with_str(&search).ok() {
        Some(p) => p,
        None => return UrlState::default(),
    };

    UrlState {
        session: params.get("session"),
        project: params.get("project").and_then(|s| s.parse().ok()),
        sidebar: params.get("sidebar").and_then(|s| parse_bool(&s)),
        terminal: params.get("terminal").and_then(|s| parse_bool(&s)),
        editor: params.get("editor").and_then(|s| parse_bool(&s)),
        git: params.get("git").and_then(|s| parse_bool(&s)),
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

// ── URL writing ────────────────────────────────────────────────────

fn build_url_params_from(
    session_id: Option<&str>,
    project_idx: Option<usize>,
    panels: &PanelState,
) -> String {
    let mut params = Vec::new();

    if let Some(sid) = session_id {
        params.push(format!("session={}", js_sys::encode_uri_component(sid)));
    }

    if let Some(idx) = project_idx {
        if idx != 0 {
            params.push(format!("project={}", idx));
        }
    }

    if !panels.sidebar_open.get_untracked() {
        params.push("sidebar=0".to_string());
    }
    if panels.terminal.open.get_untracked() {
        params.push("terminal=1".to_string());
    }
    if panels.editor.open.get_untracked() {
        params.push("editor=1".to_string());
    }
    if panels.git.open.get_untracked() {
        params.push("git=1".to_string());
    }

    if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    }
}

fn build_url_params(sse: &SseState, panels: &PanelState) -> String {
    let sid = sse.tracked_session_id();
    let project_idx = sse.app_state.get_untracked().map(|s| s.active_project);
    build_url_params_from(sid.as_deref(), project_idx, panels)
}

fn pathname() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".to_string())
}

pub fn replace_url(sse: &SseState, panels: &PanelState) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let params = build_url_params(sse, panels);
    let new_url = format!("{}{}", pathname(), params);
    let _ = window
        .history()
        .ok()
        .map(|h| h.replace_state_with_url(&JsValue::NULL, "", Some(&new_url)));
}

pub fn push_url(sse: &SseState, panels: &PanelState) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let params = build_url_params(sse, panels);
    let new_url = format!("{}{}", pathname(), params);
    let _ = window
        .history()
        .ok()
        .map(|h| h.push_state_with_url(&JsValue::NULL, "", Some(&new_url)));
}

/// Navigate to a session by updating the URL and dispatching a custom event.
/// This is the canonical way to change sessions — all other code paths
/// (sidebar click, modal select, etc.) should call this instead of directly
/// hitting backend APIs.
///
/// The `use_url_restore` hook listens for the `opman:navigate` custom event
/// and performs the actual backend select + state refresh.
pub fn navigate_to_session(session_id: &str, project_idx: usize, panels: &PanelState) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let params = build_url_params_from(Some(session_id), Some(project_idx), panels);
    let new_url = format!("{}{}", pathname(), params);
    let _ = window
        .history()
        .ok()
        .map(|h| h.push_state_with_url(&JsValue::NULL, "", Some(&new_url)));
    // Dispatch custom event so the URL watcher picks it up
    // (pushState does not fire popstate on its own)
    let _ = window.dispatch_event(&web_sys::CustomEvent::new("opman:navigate").unwrap());
}
