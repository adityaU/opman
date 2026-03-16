//! URL restore hook — URL is the **single source of truth** for session selection.
//!
//! All session navigation (sidebar click, modal, popstate, initial load) goes
//! through the URL: callers update the URL with `navigate_to_session()`, and
//! this hook reacts to URL changes to perform the backend select + state refresh.
//!
//! Split into sub-modules:
//! - `url_state`: URL parsing, writing, and `navigate_to_session()` helper

mod url_state;

pub use url_state::{navigate_to_session, UrlState};

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::SseState;
use crate::types::api::AppState;

use url_state::{parse_url_state, replace_url};

const LAST_SESSION_KEY: &str = "opman_last_session";

// ── Helpers ────────────────────────────────────────────────────────

/// Find which project contains a given session ID.
fn find_session_project(app_state: &AppState, session_id: &str) -> Option<usize> {
    app_state
        .projects
        .iter()
        .position(|p| p.sessions.iter().any(|s| s.id == session_id))
}

/// Select a session on the backend and refresh app state.
async fn select_and_refresh(
    set_app_state: WriteSignal<Option<AppState>>,
    project_idx: usize,
    active_project: usize,
    session_id: &str,
) {
    if project_idx != active_project {
        let _ = crate::api::project::switch_project(project_idx).await;
    }
    let _ = crate::api::project::select_session(project_idx, session_id).await;
    if let Ok(state) = crate::api::project::fetch_app_state().await {
        set_app_state.set(Some(state));
    }
}

/// Persist the session ID to localStorage for next-visit fallback.
fn persist_session(sid: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item(LAST_SESSION_KEY, sid);
    }
}

/// Read session ID from localStorage.
fn stored_session() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item(LAST_SESSION_KEY).ok())
        .flatten()
}

// ── Core: load session from current URL ────────────────────────────

/// Read the URL, determine desired session, and load it if different from current.
/// This is the single path that all session navigation funnels through.
fn load_session_from_url(sse: SseState, panels: PanelState) {
    let url = parse_url_state();

    // Apply panel state from URL
    if let Some(sidebar) = url.sidebar {
        if sidebar != panels.sidebar_open.get_untracked() {
            panels.toggle_sidebar();
        }
    }

    let app_state = match sse.app_state.get_untracked() {
        Some(s) => s,
        None => return,
    };
    let active_project = app_state.active_project;
    let current_sid = app_state
        .projects
        .get(active_project)
        .and_then(|p| p.active_session.as_deref())
        .map(|s| s.to_string());

    // Determine target session from URL, falling back to localStorage
    let target_sid = url.session.or_else(stored_session);

    let Some(ref sid) = target_sid else {
        // No session anywhere — nothing to load
        return;
    };

    // Already the active session — just sync URL (it may be missing session param)
    if current_sid.as_deref() == Some(sid.as_str()) {
        // Make sure URL reflects the session (for initial load where URL had nothing)
        replace_url(&sse, &panels);
        persist_session(sid);
        return;
    }

    // Find which project owns this session
    let target_project = url
        .project
        .or_else(|| find_session_project(&app_state, sid));
    let target_project = match target_project {
        Some(idx) => idx,
        None => {
            log::warn!("Session {} not found in any project", sid);
            return;
        }
    };

    // Do the switch
    let set_app_state = sse.set_app_state;
    let sid_owned = sid.clone();
    sse.expect_session_switch();
    persist_session(sid);
    leptos::task::spawn_local(async move {
        select_and_refresh(set_app_state, target_project, active_project, &sid_owned)
            .await;
    });
}

// ── Hook ───────────────────────────────────────────────────────────

/// URL restore and sync hook. Call once at layout level.
/// Returns the initial URL state parsed on first load.
pub fn use_url_restore(sse: SseState, panels: PanelState) -> UrlState {
    let initial = parse_url_state();

    // Apply initial panel state from URL immediately
    if let Some(sidebar) = initial.sidebar {
        panels.set_sidebar_open.set(sidebar);
    }
    if let Some(true) = initial.terminal {
        panels.terminal.set_open.set(true);
    }
    if let Some(true) = initial.editor {
        panels.editor.set_open.set(true);
    }
    if let Some(true) = initial.git {
        panels.git.set_open.set(true);
    }

    // ── Initial session restore ────────────────────────────────────
    // Wait until app_state is populated (sessions loaded), then load from URL.
    let restored = RwSignal::new(false);

    Effect::new(move |_| {
        if restored.get_untracked() {
            return;
        }
        // Use derived_active_project to avoid subscribing to the full app_state.
        let proj = match sse.derived_active_project.get() {
            Some(p) => p,
            None => return,
        };
        // Wait until the active project has sessions loaded
        if proj.sessions.is_empty() {
            return;
        }
        restored.set(true);
        load_session_from_url(sse, panels);
    });

    // ── Listen for programmatic navigation ─────────────────────────
    // `navigate_to_session()` dispatches "opman:navigate" after pushState.
    {
        let handler = Closure::<dyn Fn(web_sys::Event)>::new(
            move |_e: web_sys::Event| {
                load_session_from_url(sse, panels);
            },
        );
        let window = web_sys::window().unwrap();
        let _ = window.add_event_listener_with_callback(
            "opman:navigate",
            handler.as_ref().unchecked_ref(),
        );
        handler.forget();
    }

    // ── Popstate handler (back/forward) ────────────────────────────
    {
        let handler = Closure::<dyn Fn(web_sys::PopStateEvent)>::new(
            move |_e: web_sys::PopStateEvent| {
                load_session_from_url(sse, panels);
            },
        );
        let window = web_sys::window().unwrap();
        let _ = window.add_event_listener_with_callback(
            "popstate",
            handler.as_ref().unchecked_ref(),
        );
        handler.forget();
    }

    // ── Keep URL in sync when session changes via SSE ──────────────
    // (e.g. new session created server-side, state_changed event)
    // Only fires after initial restore is done.
    Effect::new(move |_| {
        let _sid = sse.tracked_session_id_reactive();
        if !restored.get_untracked() {
            return;
        }
        // Update URL to reflect current tracked session (replaceState, not push)
        replace_url(&sse, &panels);

        // Persist to localStorage
        if let Some(sid) = _sid {
            persist_session(&sid);
        }
    });

    // ── Panel changes → replaceState ───────────────────────────────
    Effect::new(move |_| {
        let _s = panels.sidebar_open.get();
        let _t = panels.terminal.open.get();
        let _e = panels.editor.open.get();
        let _g = panels.git.open.get();
        replace_url(&sse, &panels);
    });

    initial
}
