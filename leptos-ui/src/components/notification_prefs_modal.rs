//! NotificationPrefsModal — browser notification preference toggles.
//! Matches React `NotificationPrefsModal.tsx`.
//! Entirely client-side — uses localStorage for persistence.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;

// ── NotificationPrefs type ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPrefs {
    pub enabled: bool,
    pub session_complete: bool,
    pub permission_request: bool,
    pub question: bool,
    pub watcher_trigger: bool,
    pub file_edit: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            session_complete: true,
            permission_request: true,
            question: true,
            watcher_trigger: true,
            file_edit: false,
        }
    }
}

impl NotificationPrefs {
    /// Check if a specific notification kind is enabled.
    pub fn kind_enabled(&self, kind: &str) -> bool {
        match kind {
            "session_complete" => self.session_complete,
            "permission_request" => self.permission_request,
            "question" => self.question,
            "watcher_trigger" => self.watcher_trigger,
            "file_edit" => self.file_edit,
            _ => false,
        }
    }
}

pub const STORAGE_KEY: &str = "opman_notification_prefs";

pub fn load_prefs() -> NotificationPrefs {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item(STORAGE_KEY).ok())
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_prefs(prefs: &NotificationPrefs) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        if let Ok(json) = serde_json::to_string(prefs) {
            let _ = storage.set_item(STORAGE_KEY, &json);
        }
    }
}

pub fn can_notify() -> bool {
    web_sys::window()
        .and_then(|w| {
            js_sys::Reflect::get(&w, &JsValue::from_str("Notification"))
                .ok()
                .and_then(|n| {
                    js_sys::Reflect::get(&n, &JsValue::from_str("permission"))
                        .ok()
                        .and_then(|p| p.as_string())
                })
        })
        .map(|p| p == "granted")
        .unwrap_or(false)
}

// ── Pref items ──────────────────────────────────────────────────────

struct PrefItem {
    key: &'static str,
    label: &'static str,
    description: &'static str,
    icon_path: &'static str,
}

const ITEMS: &[PrefItem] = &[
    PrefItem {
        key: "session_complete",
        label: "Session Complete",
        description: "Notify when a session finishes processing",
        icon_path: "M22 11.08V12a10 10 0 1 1-5.93-9.14 M22 4 12 14.01l-3-3",
    },
    PrefItem {
        key: "permission_request",
        label: "Permission Request",
        description: "Notify when AI needs tool approval",
        icon_path: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
    },
    PrefItem {
        key: "question",
        label: "AI Question",
        description: "Notify when AI asks a question",
        icon_path: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 14v0m0-8a2 2 0 0 1 1.7 3.1L12 13",
    },
    PrefItem {
        key: "watcher_trigger",
        label: "Watcher Trigger",
        description: "Notify when a watcher auto-continues",
        icon_path: "M22 12h-4l-3 9L9 3l-3 9H2",
    },
    PrefItem {
        key: "file_edit",
        label: "File Edits",
        description: "Notify on each file edit (can be noisy)",
        icon_path: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M16 13H8 M16 17H8 M10 9H8",
    },
];

fn get_pref(prefs: &NotificationPrefs, key: &str) -> bool {
    match key {
        "session_complete" => prefs.session_complete,
        "permission_request" => prefs.permission_request,
        "question" => prefs.question,
        "watcher_trigger" => prefs.watcher_trigger,
        "file_edit" => prefs.file_edit,
        _ => false,
    }
}

fn set_pref(prefs: &mut NotificationPrefs, key: &str, val: bool) {
    match key {
        "session_complete" => prefs.session_complete = val,
        "permission_request" => prefs.permission_request = val,
        "question" => prefs.question = val,
        "watcher_trigger" => prefs.watcher_trigger = val,
        "file_edit" => prefs.file_edit = val,
        _ => {}
    }
}

#[component]
pub fn NotificationPrefsModal(
    on_close: Callback<()>,
) -> impl IntoView {
    let (prefs, set_prefs) = signal(load_prefs());
    let (permission_granted, set_permission_granted) = signal(can_notify());

    let toggle_master = move |_: web_sys::MouseEvent| {
        set_prefs.update(|p| {
            p.enabled = !p.enabled;
            save_prefs(p);
        });
    };

    let request_permission = move |_: web_sys::MouseEvent| {
        leptos::task::spawn_local(async move {
            // Use Notification API to request permission
            let result = js_sys::Reflect::get(
                &web_sys::window().unwrap(),
                &JsValue::from_str("Notification"),
            );
            if let Ok(notification_cls) = result {
                if let Ok(req_fn) = js_sys::Reflect::get(&notification_cls, &JsValue::from_str("requestPermission")) {
                    if let Ok(promise) = js_sys::Function::from(req_fn).call0(&notification_cls) {
                        let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise)).await;
                        set_permission_granted.set(can_notify());
                        if can_notify() {
                            set_prefs.update(|p| {
                                p.enabled = true;
                                save_prefs(p);
                            });
                        }
                    }
                }
            }
        });
    };

    view! {
        <ModalOverlay on_close=on_close class="notification-prefs-modal">
            <div class="notification-prefs-header">
                <h3>"Notification Preferences"</h3>
                <button on:click=move |_| on_close.run(()) aria-label="Close">
                    <IconX size=14 />
                </button>
            </div>

            <div class="notification-prefs-body">
                // Browser permission status
                <div class="notification-prefs-permission">
                    {move || {
                        if permission_granted.get() {
                            view! {
                                <div class="notification-prefs-permission-info">
                                    <span class="notification-prefs-permission-label">"Browser Permission"</span>
                                    <span class="notification-prefs-status granted">
                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                            <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
                                            <path d="M13.73 21a2 2 0 0 1-3.46 0" />
                                        </svg>
                                        " Browser notifications allowed"
                                    </span>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="notification-prefs-status denied">
                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M13.73 21a2 2 0 0 1-3.46 0" />
                                        <path d="M18.63 13A17.89 17.89 0 0 1 18 8" />
                                        <path d="M6.26 6.26A5.86 5.86 0 0 0 6 8c0 7-3 9-3 9h14" />
                                        <path d="M18 8a6 6 0 0 0-9.33-5" />
                                        <line x1="1" y1="1" x2="23" y2="23" />
                                    </svg>
                                    <span>"Browser notifications not enabled"</span>
                                    <button class="notification-prefs-grant-btn" on:click=request_permission>
                                        "Enable"
                                    </button>
                                </div>
                            }.into_any()
                        }
                    }}
                </div>

                // Master toggle
                <div
                    class="notification-prefs-item master"
                    on:click=toggle_master
                >
                    <div class="notification-prefs-item-left">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
                        </svg>
                        <div>
                            <div class="notification-prefs-item-label">"All Notifications"</div>
                            <div class="notification-prefs-item-desc">"Master toggle for all notification types"</div>
                        </div>
                    </div>
                    <span class=move || {
                        if prefs.get().enabled { "notification-prefs-badge on" } else { "notification-prefs-badge off" }
                    }>
                        {move || if prefs.get().enabled { "ON" } else { "OFF" }}
                    </span>
                </div>

                // Individual toggles
                {ITEMS.iter().map(|item| {
                    let key = item.key;
                    let label = item.label;
                    let desc = item.description;
                    let icon_path = item.icon_path;

                    let toggle_item = move |_: web_sys::MouseEvent| {
                        if prefs.get_untracked().enabled {
                            set_prefs.update(|p| {
                                let cur = get_pref(p, key);
                                set_pref(p, key, !cur);
                                save_prefs(p);
                            });
                        }
                    };

                    view! {
                        <div
                            class=move || {
                                if !prefs.get().enabled {
                                    "notification-prefs-item disabled"
                                } else {
                                    "notification-prefs-item"
                                }
                            }
                            on:click=toggle_item
                        >
                            <div class="notification-prefs-item-left">
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d=icon_path />
                                </svg>
                                <div>
                                    <div class="notification-prefs-item-label">{label}</div>
                                    <div class="notification-prefs-item-desc">{desc}</div>
                                </div>
                            </div>
                            <span class=move || {
                                if get_pref(&prefs.get(), key) { "notification-prefs-badge on" } else { "notification-prefs-badge off" }
                            }>
                                {move || if get_pref(&prefs.get(), key) { "ON" } else { "OFF" }}
                            </span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div class="notification-prefs-footer">
                "Notifications are shown when the tab is in the background."
            </div>
        </ModalOverlay>
    }
}
