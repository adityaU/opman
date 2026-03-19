//! App-level SSE listener wiring — all event listeners attached to the `/api/events` SSE.

use leptos::prelude::{GetUntracked, Set, Update};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use crate::components::toast::{ToastContext, ToastType};
use crate::hooks::use_sse_state::{ConnectionStatus, SessionStatus, SseState};
use crate::types::api::SessionStats;
use crate::types::events::WatcherStatus;

/// Wire all event listeners onto an app-level EventSource.
/// `touch_event` is called from each listener to reset the stale-connection watchdog.
pub fn wire_app_listeners(
    app_sse: &EventSource,
    sse: SseState,
    toast_ctx: Option<ToastContext>,
    touch_event: impl Fn() + Clone + 'static,
) {
    let set_connection = sse.set_connection_status;

    // Heartbeat — just touch the watchdog
    {
        let touch = touch_event.clone();
        let heartbeat_cb = Closure::<dyn Fn()>::new(move || {
            touch();
        });
        let _ = app_sse
            .add_event_listener_with_callback("heartbeat", heartbeat_cb.as_ref().unchecked_ref());
        heartbeat_cb.forget();
    }

    // state_changed — debounced full refresh via generation counter.
    {
        let set_app_state = sse.set_app_state;
        let touch = touch_event.clone();
        let state_gen = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let state_changed_cb = Closure::<dyn Fn()>::new(move || {
            touch();
            let gen = state_gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let gen_arc = state_gen.clone();
            let set_app_state = set_app_state;
            leptos::task::spawn_local(async move {
                match crate::api::api_fetch::<crate::types::api::AppState>("/state").await {
                    Ok(state) => {
                        if gen_arc.load(std::sync::atomic::Ordering::Relaxed) == gen {
                            set_app_state.set(Some(state));
                        }
                    }
                    Err(e) => log::error!("state_changed refresh failed: {}", e),
                }
            });
        });
        let _ = app_sse.add_event_listener_with_callback(
            "state_changed",
            state_changed_cb.as_ref().unchecked_ref(),
        );
        state_changed_cb.forget();
    }

    // session_busy
    {
        let set_busy = sse.set_busy_sessions;
        let set_status = sse.set_session_status;
        let sse_clone = sse;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                let sid = e.data().as_string().unwrap_or_default();
                if !sid.is_empty() {
                    let sid_clone = sid.clone();
                    set_busy.update(move |s: &mut std::collections::HashSet<String>| {
                        s.insert(sid_clone);
                    });
                    // Only update session_status if this is the tracked session and status differs
                    if sse_clone.tracked_session_id().as_deref() == Some(sid.as_str())
                        && sse_clone.session_status.get_untracked() != SessionStatus::Busy
                    {
                        set_status.set(SessionStatus::Busy);
                    }
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("session_busy", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // session_idle
    {
        let set_busy = sse.set_busy_sessions;
        let set_status = sse.set_session_status;
        let sse_clone = sse;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                let sid = e.data().as_string().unwrap_or_default();
                if !sid.is_empty() {
                    let sid_clone = sid.clone();
                    set_busy.update(move |s: &mut std::collections::HashSet<String>| {
                        s.remove(&sid_clone);
                    });
                    if sse_clone.tracked_session_id().as_deref() == Some(sid.as_str()) {
                        // Dedup: only set if not already Idle
                        if sse_clone.session_status.get_untracked() != SessionStatus::Idle {
                            set_status.set(SessionStatus::Idle);
                        }
                    }
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("session_idle", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // stats_updated
    {
        let set_stats = sse.set_stats;
        let stats_read = sse.stats;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                let data = e.data().as_string().unwrap_or_default();
                if let Ok(stats) = serde_json::from_str::<SessionStats>(&data) {
                    // Dedup: only set if actually different
                    if stats_read.get_untracked().as_ref() != Some(&stats) {
                        set_stats.set(Some(stats));
                    }
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("stats_updated", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // theme_changed
    {
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                let data = e.data().as_string().unwrap_or_default();
                if let Ok(colors) =
                    serde_json::from_str::<crate::types::api::ThemeColors>(&data)
                {
                    crate::theme::apply_theme_to_css(&colors);
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("theme_changed", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // watcher_status
    {
        let set_watcher = sse.set_watcher_status;
        let watcher_read = sse.watcher_status;
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                let data = e.data().as_string().unwrap_or_default();
                if let Ok(status) = serde_json::from_str::<WatcherStatus>(&data) {
                    let new_val = if status.action == "deleted" {
                        None
                    } else {
                        Some(status)
                    };
                    // Dedup: only set if actually different
                    if watcher_read.get_untracked() != new_val {
                        set_watcher.set(new_val);
                    }
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("watcher_status", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // toast — SSE toast events from backend/TUI
    if let Some(tctx) = toast_ctx {
        let touch = touch_event.clone();
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                touch();
                let data = e.data().as_string().unwrap_or_default();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                    let message = parsed
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    if message.is_empty() {
                        return;
                    }
                    let level_str = parsed
                        .get("level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("info");
                    let tt = match level_str {
                        "success" => ToastType::Success,
                        "error" => ToastType::Error,
                        "warning" => ToastType::Warning,
                        _ => ToastType::Info,
                    };
                    tctx.add(message, tt, 4000);
                }
            },
        );
        let _ =
            app_sse.add_event_listener_with_callback("toast", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // routine_updated — dispatch DOM event so RoutinesModal can refresh
    {
        let touch = touch_event.clone();
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |_e: web_sys::MessageEvent| {
                touch();
                if let Some(w) = web_sys::window() {
                    let _ = w.dispatch_event(
                        &web_sys::CustomEvent::new("opman:routine-updated").unwrap(),
                    );
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("routine_updated", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // presence_changed
    {
        let set_presence = sse.set_presence_clients;
        let presence_read = sse.presence_clients;
        let touch = touch_event.clone();
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                touch();
                let data = e.data().as_string().unwrap_or_default();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(clients_arr) = parsed.get("clients") {
                        if let Ok(clients) = serde_json::from_value::<
                            Vec<crate::types::api::ClientPresence>,
                        >(clients_arr.clone())
                        {
                            // Dedup: only set if actually different
                            if presence_read.get_untracked() != clients {
                                set_presence.set(clients);
                            }
                        }
                    }
                }
            },
        );
        let _ = app_sse
            .add_event_listener_with_callback("presence_changed", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // Connection status tracking (open/error) — matches React: status only, no recovery.
    // Recovery is handled by the watchdog (stale) and session SSE (error→open).
    {
        let touch_open = touch_event.clone();
        let set_conn = set_connection;
        let sse_for_open = sse;

        let open_cb = Closure::<dyn Fn()>::new(move || {
            touch_open();
            if sse_for_open.connection_status.get_untracked() != ConnectionStatus::Connected {
                set_conn.set(ConnectionStatus::Connected);
            }
        });
        let _ = app_sse
            .add_event_listener_with_callback("open", open_cb.as_ref().unchecked_ref());
        open_cb.forget();

        let set_conn2 = set_connection;
        let sse_for_err = sse;
        let err_cb = Closure::<dyn Fn()>::new(move || {
            log::warn!(
                "[SSE] App events connection error — EventSource will auto-reconnect"
            );
            if sse_for_err.connection_status.get_untracked() != ConnectionStatus::Reconnecting {
                set_conn2.set(ConnectionStatus::Reconnecting);
            }
        });
        let _ = app_sse
            .add_event_listener_with_callback("error", err_cb.as_ref().unchecked_ref());
        err_cb.forget();
    }
}
