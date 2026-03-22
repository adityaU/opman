//! Session indicator SSE listeners — busy/idle/error/input state transitions.
//!
//! Extracted from `app_listeners` to keep files under 300 lines.

use leptos::prelude::{GetUntracked, Set, Update};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use crate::hooks::use_sse_state::{SessionStatus, SseState};

/// Wire session-indicator listeners onto the app-level EventSource.
pub fn wire_indicator_listeners(app_sse: &EventSource, sse: SseState) {
    // session_busy — mark session as busy, clear error state
    {
        let set_busy = sse.set_busy_sessions;
        let set_error = sse.set_error_sessions;
        let set_status = sse.set_session_status;
        let sse_clone = sse;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let sid = e.data().as_string().unwrap_or_default();
            if sid.is_empty() {
                return;
            }
            let sid_busy = sid.clone();
            let sid_err = sid.clone();
            set_busy.update(move |s: &mut std::collections::HashSet<String>| {
                s.insert(sid_busy);
            });
            set_error.update(move |s: &mut std::collections::HashSet<String>| {
                s.remove(&sid_err);
            });
            if sse_clone.tracked_session_id().as_deref() == Some(sid.as_str())
                && sse_clone.session_status.get_untracked() != SessionStatus::Busy
            {
                set_status.set(SessionStatus::Busy);
            }
        });
        let _ =
            app_sse.add_event_listener_with_callback("session_busy", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_idle — clear busy + input state
    {
        let set_busy = sse.set_busy_sessions;
        let set_input = sse.set_input_sessions;
        let set_status = sse.set_session_status;
        let sse_clone = sse;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let sid = e.data().as_string().unwrap_or_default();
            if sid.is_empty() {
                return;
            }
            let sid_busy = sid.clone();
            let sid_input = sid.clone();
            set_busy.update(move |s: &mut std::collections::HashSet<String>| {
                s.remove(&sid_busy);
            });
            set_input.update(move |s: &mut std::collections::HashSet<String>| {
                s.remove(&sid_input);
            });
            if sse_clone.tracked_session_id().as_deref() == Some(sid.as_str())
                && sse_clone.session_status.get_untracked() != SessionStatus::Idle
            {
                set_status.set(SessionStatus::Idle);
            }
        });
        let _ =
            app_sse.add_event_listener_with_callback("session_idle", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_error — mark session as errored
    {
        let set_error = sse.set_error_sessions;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let data = e.data().as_string().unwrap_or_default();
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) else {
                return;
            };
            let Some(sid) = parsed.get("session_id").and_then(|v| v.as_str()) else {
                return;
            };
            let sid = sid.to_string();
            set_error.update(move |s: &mut std::collections::HashSet<String>| {
                s.insert(sid);
            });
        });
        let _ =
            app_sse.add_event_listener_with_callback("session_error", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_input_needed — mark session as needing input (permission/question)
    {
        let set_input = sse.set_input_sessions;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let data = e.data().as_string().unwrap_or_default();
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) else {
                return;
            };
            let Some(sid) = parsed.get("session_id").and_then(|v| v.as_str()) else {
                return;
            };
            let sid = sid.to_string();
            set_input.update(move |s: &mut std::collections::HashSet<String>| {
                s.insert(sid);
            });
        });
        let _ = app_sse
            .add_event_listener_with_callback("session_input_needed", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_input_cleared — remove session from input-needed set
    {
        let set_input = sse.set_input_sessions;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let data = e.data().as_string().unwrap_or_default();
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) else {
                return;
            };
            let Some(sid) = parsed.get("session_id").and_then(|v| v.as_str()) else {
                return;
            };
            let sid = sid.to_string();
            set_input.update(move |s: &mut std::collections::HashSet<String>| {
                s.remove(&sid);
            });
        });
        let _ = app_sse
            .add_event_listener_with_callback("session_input_cleared", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_unseen — mark session as having unseen activity
    {
        let set_unseen = sse.set_unseen_sessions;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let data = e.data().as_string().unwrap_or_default();
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) else {
                return;
            };
            let Some(sid) = parsed.get("session_id").and_then(|v| v.as_str()) else {
                return;
            };
            let sid = sid.to_string();
            set_unseen.update(move |s: &mut std::collections::HashSet<String>| {
                s.insert(sid);
            });
        });
        let _ =
            app_sse.add_event_listener_with_callback("session_unseen", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_seen — clear unseen state for a session
    {
        let set_unseen = sse.set_unseen_sessions;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let data = e.data().as_string().unwrap_or_default();
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) else {
                return;
            };
            let Some(sid) = parsed.get("session_id").and_then(|v| v.as_str()) else {
                return;
            };
            let sid = sid.to_string();
            set_unseen.update(move |s: &mut std::collections::HashSet<String>| {
                s.remove(&sid);
            });
        });
        let _ =
            app_sse.add_event_listener_with_callback("session_seen", cb.as_ref().unchecked_ref());
        cb.forget();
    }
}
