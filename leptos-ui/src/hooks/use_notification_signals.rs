//! Notification signals hook — presence heartbeat + browser notifications.
//! Matches React `useNotificationSignals.ts` + `NotificationManager.ts`.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::notification_prefs_modal::{can_notify, load_prefs, NotificationPrefs};
use crate::hooks::use_sse_state::{SessionStatus, SseState};

// ── Types ──────────────────────────────────────────────────────────

/// An assistant signal event (client-side event log).
#[derive(Debug, Clone)]
pub struct AssistantSignal {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub timestamp: f64,
    pub session_id: Option<String>,
}

// ── Stable client ID (per-tab via sessionStorage) ──────────────────

/// Generate / retrieve a stable client ID for this browser tab.
/// Persisted in `sessionStorage` (per-tab, not shared between tabs).
fn get_client_id() -> String {
    let key = "opman_client_id";
    let storage = web_sys::window()
        .and_then(|w| w.session_storage().ok())
        .flatten();

    if let Some(ref s) = storage {
        if let Ok(Some(id)) = s.get_item(key) {
            return id;
        }
    }

    // Try crypto.randomUUID(), fallback to Math.random
    let uuid: String = js_sys::Reflect::get(
        &web_sys::window().unwrap().navigator(),
        &JsValue::from_str("crypto"),
    )
    .ok()
    .and_then(|crypto| {
        js_sys::Reflect::get(&crypto, &JsValue::from_str("randomUUID")).ok()
    })
    .and_then(|func| func.dyn_ref::<js_sys::Function>().cloned())
    .and_then(|func| {
        func.call0(
            &web_sys::window()
                .unwrap()
                .navigator()
                .unchecked_into::<JsValue>(),
        )
        .ok()
    })
    .and_then(|v| v.as_string())
    .unwrap_or_else(|| {
        // Fallback: random string
        js_sys::Math::random().to_string().chars().skip(2).take(12).collect()
    });

    let id = format!("web-{}", uuid);
    if let Some(ref s) = storage {
        let _ = s.set_item(key, &id);
    }
    id
}

// ── Show notification (matching NotificationManager.ts) ────────────

/// Show a browser notification if the tab is hidden and the event kind is enabled.
/// Prefers service-worker path, falls back to plain Notification API.
fn show_notification(
    kind: &str,
    title: &str,
    body: &str,
    prefs: &NotificationPrefs,
    session_id: Option<&str>,
) {
    // Only notify when tab is hidden (user is away)
    let document = match web_sys::window().and_then(|w| w.document()) {
        Some(d) => d,
        None => return,
    };
    if !document.hidden() {
        return;
    }
    if !prefs.enabled {
        return;
    }
    if !prefs.kind_enabled(kind) {
        return;
    }
    if !can_notify() {
        return;
    }

    let tag = format!("opman-{}-{}", kind, js_sys::Date::now() as u64);

    // ── Try service-worker path first ──
    let sw_container = web_sys::window().map(|w| w.navigator().service_worker());
    let has_controller = sw_container
        .as_ref()
        .and_then(|sw| {
            js_sys::Reflect::get(sw, &JsValue::from_str("controller"))
                .ok()
        })
        .map(|v| !v.is_null() && !v.is_undefined())
        .unwrap_or(false);

    if has_controller {
        if let Some(ref sw) = sw_container {
            if let Ok(controller) =
                js_sys::Reflect::get(sw, &JsValue::from_str("controller"))
            {
                if let Ok(post_fn) =
                    js_sys::Reflect::get(&controller, &JsValue::from_str("postMessage"))
                {
                    if let Some(func) = post_fn.dyn_ref::<js_sys::Function>() {
                        let payload = js_sys::Object::new();
                        let _ = js_sys::Reflect::set(
                            &payload,
                            &JsValue::from_str("type"),
                            &JsValue::from_str("SHOW_NOTIFICATION"),
                        );
                        let inner = js_sys::Object::new();
                        let _ = js_sys::Reflect::set(
                            &inner,
                            &JsValue::from_str("title"),
                            &JsValue::from_str(title),
                        );
                        let _ = js_sys::Reflect::set(
                            &inner,
                            &JsValue::from_str("body"),
                            &JsValue::from_str(body),
                        );
                        let _ = js_sys::Reflect::set(
                            &inner,
                            &JsValue::from_str("tag"),
                            &JsValue::from_str(&tag),
                        );
                        let _ = js_sys::Reflect::set(
                            &inner,
                            &JsValue::from_str("kind"),
                            &JsValue::from_str(kind),
                        );
                        let _ = js_sys::Reflect::set(
                            &inner,
                            &JsValue::from_str("sessionId"),
                            &session_id
                                .map(|s| JsValue::from_str(s))
                                .unwrap_or(JsValue::NULL),
                        );
                        let _ = js_sys::Reflect::set(
                            &inner,
                            &JsValue::from_str("url"),
                            &JsValue::from_str("/"),
                        );
                        let _ = js_sys::Reflect::set(
                            &payload,
                            &JsValue::from_str("payload"),
                            &inner,
                        );
                        let _ = func.call1(&controller, &payload);
                        return;
                    }
                }
            }
        }
    }

    // ── Fallback: plain Notification API ──
    let opts = web_sys::NotificationOptions::new();
    opts.set_body(body);
    opts.set_icon("/favicon.svg");
    opts.set_tag(&tag);
    opts.set_silent(Some(false));

    if let Ok(notification) = web_sys::Notification::new_with_options(title, &opts) {
        // onclick: focus window + close notification
        let notif_clone = notification.clone();
        let onclick = Closure::<dyn Fn()>::new(move || {
            if let Some(window) = web_sys::window() {
                let _ = window.focus();
            }
            notif_clone.close();
        });
        notification.set_onclick(Some(onclick.as_ref().unchecked_ref()));
        onclick.forget();

        // Auto-close after 8 seconds
        let notif_close = notification.clone();
        let timeout_cb = Closure::<dyn Fn()>::new(move || {
            notif_close.close();
        });
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_cb.as_ref().unchecked_ref(),
                8000,
            );
        }
        timeout_cb.forget();
    }
}

// ── Helper: push signal and cap at 25 ──────────────────────────────

fn push_signal(set_signals: &WriteSignal<Vec<AssistantSignal>>, sig: AssistantSignal) {
    set_signals.update(|s| {
        s.insert(0, sig);
        s.truncate(25);
    });
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create notification signal handling. Call once at layout level.
/// Returns signals for assistant signals and a dismiss callback.
pub fn use_notification_signals(
    sse: SseState,
    autonomy_mode: ReadSignal<String>,
) -> (
    ReadSignal<Vec<AssistantSignal>>,
    WriteSignal<Vec<AssistantSignal>>,
) {
    let (signals, set_signals) = signal(Vec::<AssistantSignal>::new());

    // ── Stable client ID (per-tab via sessionStorage) ──
    let client_id = get_client_id();

    // ── Presence registration + heartbeat ──
    // Re-run when active_session_id changes (tracked via derived_active_project).
    {
        let cid = client_id.clone();
        Effect::new(move |_| {
            // Track derived_active_project to re-run when active session changes
            // (narrower than full app_state).
            let _proj = sse.derived_active_project.get();
            let sid = sse.active_session_id();
            let cid = cid.clone();

            // Register immediately
            {
                let cid_reg = cid.clone();
                let sid_reg = sid.clone();
                leptos::task::spawn_local(async move {
                    let _ = crate::api::client::api_post_void(
                        "/presence",
                        &serde_json::json!({
                            "client_id": cid_reg,
                            "interface_type": "web",
                            "focused_session": sid_reg,
                        }),
                    )
                    .await;
                });
            }

            // Heartbeat every 30s
            let cid_hb = cid.clone();
            let sse_copy = sse;
            let handler = Closure::<dyn Fn()>::new(move || {
                let cid = cid_hb.clone();
                let s = sse_copy;
                leptos::task::spawn_local(async move {
                    let sid = s.active_session_id();
                    let _ = crate::api::client::api_post_void(
                        "/presence",
                        &serde_json::json!({
                            "client_id": cid,
                            "interface_type": "web",
                            "focused_session": sid,
                        }),
                    )
                    .await;
                });
            });
            let window = web_sys::window().unwrap();
            let interval_id = window
                .set_interval_with_callback_and_timeout_and_arguments_0(
                    handler.as_ref().unchecked_ref(),
                    30_000,
                )
                .unwrap_or(0);
            handler.forget();

            // Cleanup: clear interval + deregister presence
            let cid_cleanup = cid.clone();
            on_cleanup(move || {
                if let Some(window) = web_sys::window() {
                    window.clear_interval_with_handle(interval_id);
                }
                let cid_dereg = cid_cleanup.clone();
                leptos::task::spawn_local(async move {
                    let _ = crate::api::client::api_delete_with_body(
                        "/presence",
                        &serde_json::json!({ "client_id": cid_dereg }),
                    )
                    .await;
                });
            });
        });
    }

    // ── Session completion notifications ──
    // Track previous status AND previous session ID for same-session guard.
    {
        let prev_status = RwSignal::new(SessionStatus::Idle);
        let prev_session_id = RwSignal::new(Option::<String>::None);
        Effect::new(move |_| {
            let status = sse.session_status.get();
            let current_sid = sse.active_session_id();
            let prev_s = prev_status.get_untracked();
            let prev_sid = prev_session_id.get_untracked();

            // Guard: only write when value actually changed
            if prev_s != status {
                prev_status.set(status);
            }
            if prev_sid != current_sid {
                prev_session_id.set(current_sid.clone());
            }

            let prefs = load_prefs();
            if !prefs.enabled {
                return;
            }

            // Only fire when same session transitioned busy→idle
            let was_busy = prev_s == SessionStatus::Busy;
            let same_session = prev_sid == current_sid;

            if status == SessionStatus::Idle
                && was_busy
                && same_session
                && prefs.session_complete
                && autonomy_mode.get_untracked() != "observe"
            {
                let sid_str = current_sid
                    .as_deref()
                    .unwrap_or("none")
                    .to_string();
                let now = js_sys::Date::now();
                let sig = AssistantSignal {
                    id: format!("session-complete:{}:{}", sid_str, now as u64),
                    kind: "session_complete".to_string(),
                    summary: "AI session has finished processing".to_string(),
                    timestamp: now / 1000.0,
                    session_id: current_sid.clone(),
                };
                push_signal(&set_signals, sig);
                show_notification(
                    "session_complete",
                    "Session Complete",
                    "AI session has finished processing",
                    &prefs,
                    current_sid.as_deref(),
                );
            }
        });
    }

    // ── Watcher-triggered signals + notifications ──
    {
        Effect::new(move |_| {
            let ws = sse.watcher_status.get();
            let ws = match ws {
                Some(w) => w,
                None => return,
            };
            if ws.action != "triggered" {
                return;
            }
            if autonomy_mode.get_untracked() == "observe" {
                return;
            }

            let prefs = load_prefs();
            let now = js_sys::Date::now();
            let sig = AssistantSignal {
                id: format!("watcher-trigger:{}:{}", ws.session_id, now as u64),
                kind: "watcher_trigger".to_string(),
                summary: "A watched session auto-continued and may need review.".to_string(),
                timestamp: now / 1000.0,
                session_id: Some(ws.session_id.clone()),
            };
            push_signal(&set_signals, sig);
            show_notification(
                "watcher_trigger",
                "Watcher Triggered",
                "A watched session auto-continued and may need review.",
                &prefs,
                Some(&ws.session_id),
            );
        });
    }

    // ── Permission request notifications (ID-based dedup) ──
    {
        let notified_perm_ids: Rc<RefCell<HashSet<String>>> =
            Rc::new(RefCell::new(HashSet::new()));
        let ids = notified_perm_ids.clone();
        Effect::new(move |_| {
            let perms = sse.permissions.get();
            let prefs = load_prefs();

            for perm in perms.iter() {
                let mut seen = ids.borrow_mut();
                if seen.contains(&perm.id) {
                    continue;
                }
                seen.insert(perm.id.clone());
                drop(seen);

                let label = if perm.tool_name.is_empty() {
                    "Permission".to_string()
                } else {
                    perm.tool_name.clone()
                };
                let now = js_sys::Date::now();
                let sig = AssistantSignal {
                    id: format!("permission-request:{}:{}", perm.id, now as u64),
                    kind: "permission_request".to_string(),
                    summary: format!("Tool \"{}\" needs approval", label),
                    timestamp: now / 1000.0,
                    session_id: Some(perm.session_id.clone()),
                };
                push_signal(&set_signals, sig);
                show_notification(
                    "permission_request",
                    "Permission Requested",
                    &format!("Tool \"{}\" needs approval", label),
                    &prefs,
                    Some(&perm.session_id),
                );
            }
        });
    }

    // ── Question notifications (ID-based dedup) ──
    {
        let notified_q_ids: Rc<RefCell<HashSet<String>>> =
            Rc::new(RefCell::new(HashSet::new()));
        let ids = notified_q_ids.clone();
        Effect::new(move |_| {
            let questions = sse.questions.get();
            let prefs = load_prefs();

            for q in questions.iter() {
                let mut seen = ids.borrow_mut();
                if seen.contains(&q.id) {
                    continue;
                }
                seen.insert(q.id.clone());
                drop(seen);

                let label = if q.title.is_empty() {
                    "Question".to_string()
                } else {
                    q.title.clone()
                };
                let now = js_sys::Date::now();
                let sig = AssistantSignal {
                    id: format!("question:{}:{}", q.id, now as u64),
                    kind: "question".to_string(),
                    summary: label.clone(),
                    timestamp: now / 1000.0,
                    session_id: Some(q.session_id.clone()),
                };
                push_signal(&set_signals, sig);
                show_notification(
                    "question",
                    "AI Question",
                    &label,
                    &prefs,
                    Some(&q.session_id),
                );
            }
        });
    }

    // ── File edit notifications ──
    // No assistant signal for file edits (too noisy) — just browser notification.
    {
        let prev_edit_count = RwSignal::new(0usize);
        Effect::new(move |_| {
            let count = sse.file_edit_count.get();
            let prev = prev_edit_count.get_untracked();

            // Guard: only write when value actually changed
            if prev != count {
                prev_edit_count.set(count);
            }

            // Only fire when count increases AND not on initial mount (prev == 0 && count == 0)
            if count <= prev {
                return;
            }
            // Don't fire on the very first mount where prev was 0
            if prev == 0 && count == 0 {
                return;
            }

            let prefs = load_prefs();
            let sid = sse.active_session_id();
            show_notification(
                "file_edit",
                "File Edited",
                &format!("{} file(s) edited in the current session", count - prev),
                &prefs,
                sid.as_deref(),
            );
        });
    }

    // ── Listen for NOTIFICATION_CLICK messages from the service worker ──
    // Uses addEventListener (not .onmessage) + on_cleanup to remove.
    {
        Effect::new(move |_| {
            let handler = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
                move |e: web_sys::MessageEvent| {
                    let data = e.data();
                    if let Some(typ) = js_sys::Reflect::get(&data, &JsValue::from_str("type"))
                        .ok()
                        .and_then(|v| v.as_string())
                    {
                        if typ == "NOTIFICATION_CLICK" {
                            if let Some(window) = web_sys::window() {
                                let _ = window.focus();
                            }
                        }
                    }
                },
            );

            let handler_fn: js_sys::Function =
                handler.as_ref().unchecked_ref::<js_sys::Function>().clone();

            if let Some(sw) = web_sys::window().map(|w| w.navigator().service_worker()) {
                let _ = sw.add_event_listener_with_callback("message", &handler_fn);
            }

            // Leak the closure so the JS function reference stays valid.
            // The listener is removed via on_cleanup using the cloned Function handle.
            handler.forget();

            on_cleanup(move || {
                if let Some(sw) = web_sys::window().map(|w| w.navigator().service_worker()) {
                    let _ = sw.remove_event_listener_with_callback("message", &handler_fn);
                }
            });
        });
    }

    (signals, set_signals)
}
