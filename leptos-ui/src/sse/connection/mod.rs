//! SSE connection management — EventSource creation, event parsing, and wiring.
//! Includes stale-connection watchdog that closes/recreates EventSources (matches React).
//!
//! Sub-modules:
//! - `event_handler`: opencode event dispatch (message/session/permission/question)
//! - `app_listeners`: app-level SSE listener wiring
//! - `app_indicator_listeners`: session indicator listeners (busy/idle/error/input)

mod app_indicator_listeners;
mod app_listeners;
mod event_handler;
pub mod session_handlers;

use leptos::prelude::Set;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use crate::components::toast::ToastContext;
use crate::hooks::use_sse_state::{ConnectionStatus, SseState};

/// Stale-connection threshold (ms). If no events for this long, connections are stale.
const STALE_THRESHOLD_MS: u64 = 45_000;
/// Watchdog check interval (ms). React uses 10s.
const WATCHDOG_INTERVAL_MS: i32 = 10_000;

/// Create an EventSource for app-level events.
pub fn create_events_sse() -> Result<EventSource, String> {
    EventSource::new("/api/events").map_err(|e| format!("Failed to create events SSE: {:?}", e))
}

/// Create an EventSource for session-level events (opencode proxied).
pub fn create_session_events_sse() -> Result<EventSource, String> {
    EventSource::new("/api/session/events")
        .map_err(|e| format!("Failed to create session events SSE: {:?}", e))
}

/// Create an EventSource for PTY output streaming.
pub fn create_pty_sse(id: &str) -> Result<EventSource, String> {
    let url = format!("/api/pty/stream?id={}", js_sys::encode_uri_component(id));
    EventSource::new(&url).map_err(|e| format!("Failed to create PTY SSE: {:?}", e))
}

/// Create an EventSource for system stats streaming.
pub fn create_system_stats_sse() -> Result<EventSource, String> {
    EventSource::new("/api/system/stats/stream")
        .map_err(|e| format!("Failed to create system stats SSE: {:?}", e))
}

/// Create an EventSource for editor file-change events.
pub fn create_editor_events_sse() -> Result<EventSource, String> {
    EventSource::new("/api/editor/events")
        .map_err(|e| format!("Failed to create editor events SSE: {:?}", e))
}

/// Parse an opencode SSE event from JSON string.
pub fn parse_opencode_event(data: &str) -> Option<serde_json::Value> {
    let raw: serde_json::Value = serde_json::from_str(data).ok()?;

    // Try wrapped format: { payload: { type, properties } }
    if let Some(payload) = raw.get("payload") {
        if payload.get("type").is_some() {
            return Some(payload.clone());
        }
    }

    // Try direct format: { type, properties }
    if raw.get("type").is_some() {
        return Some(raw);
    }

    None
}

/// Recovery after an SSE reconnect: refresh app state, messages, and pending items.
fn recover_after_reconnect(sse: SseState) {
    log::info!("[SSE] Recovering after reconnection");
    let set_app_state = sse.set_app_state;
    leptos::task::spawn_local(async move {
        match crate::api::api_fetch::<crate::types::api::AppState>("/state").await {
            Ok(state) => set_app_state.set(Some(state)),
            Err(e) => log::error!("reconnect recovery: state fetch failed: {}", e),
        }
    });
    sse.refresh_messages();
    sse.hydrate_pending();
}

type SseSlot = Rc<RefCell<Option<EventSource>>>;

/// Wire session-level SSE listeners onto the given EventSource.
fn wire_session_listeners(
    session_sse: &EventSource,
    sse: SseState,
    touch_event: impl Fn() + Clone + 'static,
) {
    let set_connection = sse.set_connection_status;

    // opencode event
    {
        let touch = touch_event.clone();
        let cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                touch();
                let data = e.data().as_string().unwrap_or_default();
                if let Some(event) = parse_opencode_event(&data) {
                    event_handler::handle_opencode_event(&sse, &event);
                }
            },
        );
        let _ = session_sse
            .add_event_listener_with_callback("opencode", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    // heartbeat
    {
        let touch = touch_event.clone();
        let hb_cb = Closure::<dyn Fn()>::new(move || {
            touch();
        });
        let _ = session_sse
            .add_event_listener_with_callback("heartbeat", hb_cb.as_ref().unchecked_ref());
        hb_cb.forget();
    }

    // lagged — trigger recovery
    {
        let sse_for_lag = sse;
        let lag_cb = Closure::<dyn Fn()>::new(move || {
            log::warn!("[SSE] Session events lagged — recovering");
            recover_after_reconnect(sse_for_lag);
        });
        let _ = session_sse
            .add_event_listener_with_callback("lagged", lag_cb.as_ref().unchecked_ref());
        lag_cb.forget();
    }

    // error/open — session SSE reconnect recovery (matches React sessionSseNeedsRecovery)
    {
        let needs_recovery =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let needs_recovery_open = needs_recovery.clone();
        let touch_open = touch_event;

        let err_cb = Closure::<dyn Fn()>::new(move || {
            log::warn!("[SSE] Session events connection error");
            needs_recovery.store(true, std::sync::atomic::Ordering::Relaxed);
            set_connection.set(ConnectionStatus::Reconnecting);
        });
        let _ = session_sse
            .add_event_listener_with_callback("error", err_cb.as_ref().unchecked_ref());
        err_cb.forget();

        let open_cb = Closure::<dyn Fn()>::new(move || {
            touch_open();
            if needs_recovery_open.swap(false, std::sync::atomic::Ordering::Relaxed) {
                recover_after_reconnect(sse);
            }
        });
        let _ = session_sse
            .add_event_listener_with_callback("open", open_cb.as_ref().unchecked_ref());
        open_cb.forget();
    }
}

/// Wire up both SSE connections (app events + session events) and route events to SseState.
/// Includes a stale-connection watchdog that closes and recreates EventSources (matches React).
/// Call this once in ChatLayout after SseState is created.
pub fn wire_sse(sse: SseState, toast_ctx: Option<ToastContext>) {
    let set_connection = sse.set_connection_status;

    // Shared timestamp for stale-connection watchdog
    let last_event_time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
        js_sys::Date::now() as u64,
    ));

    let touch_event = {
        let last = last_event_time.clone();
        move || {
            last.store(
                js_sys::Date::now() as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    };

    // Mutable slots so the watchdog can close/recreate connections
    let app_slot: SseSlot = Rc::new(RefCell::new(None));
    let session_slot: SseSlot = Rc::new(RefCell::new(None));

    // Create initial app SSE
    match create_events_sse() {
        Ok(es) => {
            app_listeners::wire_app_listeners(&es, sse, toast_ctx.clone(), touch_event.clone());
            *app_slot.borrow_mut() = Some(es);
        }
        Err(e) => {
            log::error!("Failed to create app SSE: {}", e);
            set_connection.set(ConnectionStatus::Disconnected);
        }
    }

    // Create initial session SSE
    match create_session_events_sse() {
        Ok(es) => {
            wire_session_listeners(&es, sse, touch_event.clone());
            *session_slot.borrow_mut() = Some(es);
        }
        Err(e) => {
            log::error!("Failed to create session SSE: {}", e);
        }
    }

    // ── Stale-connection watchdog ──
    // Checks every 10s. If no events in 45s, close and recreate both EventSources.
    {
        let last = last_event_time;
        let app = app_slot;
        let session = session_slot;
        let touch = touch_event;
        let toast = toast_ctx;
        let cb = Closure::<dyn Fn()>::wrap(Box::new(move || {
            let now = js_sys::Date::now() as u64;
            let last_ts = last.load(std::sync::atomic::Ordering::Relaxed);
            if now.saturating_sub(last_ts) <= STALE_THRESHOLD_MS {
                return;
            }
            let elapsed_s = (now - last_ts) / 1000;
            log::warn!("[SSE] No events in {}s — closing and recreating EventSources", elapsed_s);
            set_connection.set(ConnectionStatus::Reconnecting);

            // Close stale connections
            if let Some(old) = app.borrow_mut().take() {
                old.close();
            }
            if let Some(old) = session.borrow_mut().take() {
                old.close();
            }

            // Recreate app SSE
            match create_events_sse() {
                Ok(es) => {
                    app_listeners::wire_app_listeners(&es, sse, toast.clone(), touch.clone());
                    *app.borrow_mut() = Some(es);
                }
                Err(e) => log::error!("Failed to recreate app SSE: {}", e),
            }

            // Recreate session SSE
            match create_session_events_sse() {
                Ok(es) => {
                    wire_session_listeners(&es, sse, touch.clone());
                    *session.borrow_mut() = Some(es);
                }
                Err(e) => log::error!("Failed to recreate session SSE: {}", e),
            }

            last.store(now, std::sync::atomic::Ordering::Relaxed);
            recover_after_reconnect(sse);
        }));
        let window = web_sys::window().unwrap();
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            WATCHDOG_INTERVAL_MS,
        );
        cb.forget();
    }
}
