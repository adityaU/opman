//! SSE connection management — EventSource creation, event parsing, and wiring.
//! Includes stale-connection watchdog and reconnect recovery.
//!
//! Split into sub-modules:
//! - `event_handler`: opencode event dispatch (message/session/permission/question)
//! - `app_listeners`: app-level SSE listener wiring

mod app_listeners;
mod event_handler;
mod session_handlers;

use leptos::prelude::Set;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::EventSource;

use crate::components::toast::ToastContext;
use crate::hooks::use_sse_state::{ConnectionStatus, SseState};

/// Stale-connection timeout in milliseconds (~45s matches React).
const STALE_TIMEOUT_MS: i32 = 45_000;

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

/// Recovery after an SSE reconnect: refresh app state and messages.
fn recover_after_reconnect(sse: SseState) {
    let set_app_state = sse.set_app_state;
    leptos::task::spawn_local(async move {
        match crate::api::api_fetch::<crate::types::api::AppState>("/state").await {
            Ok(state) => {
                set_app_state.set(Some(state));
                sse.refresh_messages();
            }
            Err(e) => log::error!("reconnect recovery failed: {}", e),
        }
    });
}

/// Wire up both SSE connections (app events + session events) and route events to SseState.
/// Includes stale-connection watchdog (~45s).
/// Call this once in ChatLayout after SseState is created.
pub fn wire_sse(sse: SseState, toast_ctx: Option<ToastContext>) {
    let set_connection = sse.set_connection_status;

    // ── Stale-connection watchdog ──
    let last_event_time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
        js_sys::Date::now() as u64,
    ));
    let last_event_clone = last_event_time.clone();

    let touch_event = {
        let last = last_event_time.clone();
        move || {
            last.store(
                js_sys::Date::now() as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    };

    // Watchdog interval
    {
        let set_conn = set_connection;
        let last = last_event_clone;
        let sse_for_recovery = sse;
        let cb = Closure::<dyn Fn()>::wrap(Box::new(move || {
            let now = js_sys::Date::now() as u64;
            let last_ts = last.load(std::sync::atomic::Ordering::Relaxed);
            if now.saturating_sub(last_ts) > STALE_TIMEOUT_MS as u64 {
                log::warn!("SSE stale connection detected — triggering recovery");
                set_conn.set(ConnectionStatus::Reconnecting);
                recover_after_reconnect(sse_for_recovery);
                last.store(now, std::sync::atomic::Ordering::Relaxed);
            }
        }));
        let window = web_sys::window().unwrap();
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            STALE_TIMEOUT_MS,
        );
        cb.forget();
    }

    // ── App-level SSE ──
    match create_events_sse() {
        Ok(app_sse) => {
            app_listeners::wire_app_listeners(&app_sse, sse, toast_ctx, touch_event.clone());
            std::mem::forget(app_sse);
        }
        Err(e) => {
            log::error!("Failed to create app SSE: {}", e);
            set_connection.set(ConnectionStatus::Disconnected);
        }
    }

    // ── Session-level SSE (opencode events) ──
    match create_session_events_sse() {
        Ok(session_sse) => {
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
                let _ = session_sse.add_event_listener_with_callback(
                    "opencode",
                    cb.as_ref().unchecked_ref(),
                );
                cb.forget();
            }

            // heartbeat
            {
                let touch = touch_event.clone();
                let hb_cb = Closure::<dyn Fn()>::new(move || {
                    touch();
                });
                let _ = session_sse.add_event_listener_with_callback(
                    "heartbeat",
                    hb_cb.as_ref().unchecked_ref(),
                );
                hb_cb.forget();
            }

            // lagged — trigger recovery
            {
                let sse_for_lag = sse;
                let lag_cb = Closure::<dyn Fn()>::new(move || {
                    log::warn!("Session events lagged — recovering");
                    recover_after_reconnect(sse_for_lag);
                });
                let _ = session_sse.add_event_listener_with_callback(
                    "lagged",
                    lag_cb.as_ref().unchecked_ref(),
                );
                lag_cb.forget();
            }

            std::mem::forget(session_sse);
        }
        Err(e) => {
            log::error!("Failed to create session SSE: {}", e);
        }
    }
}
