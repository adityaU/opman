//! WatcherModal — session watcher configuration.
//! Matches React `watcher-modal/WatcherModal.tsx` with SessionList + ConfigForm.

use leptos::prelude::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::api::watchers::{
    self, WatcherConfigRequest, WatcherMessageEntry,
};
use crate::types::api::{WatcherConfigResponse, WatcherSessionEntry};
use crate::components::icons::*;

// ── Session grouping ────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum SessionGroup {
    Current,
    Watched,
    Active,
    Other,
}

impl SessionGroup {
    fn label(&self) -> &'static str {
        match self {
            Self::Current => "Current Session",
            Self::Watched => "Watched",
            Self::Active => "Active Sessions",
            Self::Other => "Other Sessions",
        }
    }
}

fn classify_session(s: &WatcherSessionEntry) -> SessionGroup {
    if s.is_current {
        SessionGroup::Current
    } else if s.has_watcher {
        SessionGroup::Watched
    } else if s.is_active {
        SessionGroup::Active
    } else {
        SessionGroup::Other
    }
}

fn group_sessions(sessions: &[WatcherSessionEntry]) -> Vec<(SessionGroup, Vec<WatcherSessionEntry>)> {
    let order = [SessionGroup::Current, SessionGroup::Watched, SessionGroup::Active, SessionGroup::Other];
    let mut result = Vec::new();
    for group in &order {
        let items: Vec<_> = sessions.iter()
            .filter(|s| classify_session(s) == *group)
            .cloned()
            .collect();
        if !items.is_empty() {
            result.push((group.clone(), items));
        }
    }
    result
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn WatcherModal(
    on_close: Callback<()>,
) -> impl IntoView {
    // State signals
    let (sessions, set_sessions) = signal::<Vec<WatcherSessionEntry>>(Vec::new());
    let (selected_id, set_selected_id) = signal::<Option<String>>(None);
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    // Form state
    let (continuation_msg, set_continuation_msg) = signal("Continue.".to_string());
    let (idle_timeout, set_idle_timeout) = signal(10u64);
    let (include_original, set_include_original) = signal(false);
    let (original_message, set_original_message) = signal::<Option<String>>(None);
    let (hang_message, set_hang_message) = signal(
        "The previous attempt appears to have stalled. Please retry the task.".to_string()
    );
    let (hang_timeout, set_hang_timeout) = signal(180u64);
    let (existing_watcher, set_existing_watcher) = signal::<Option<WatcherConfigResponse>>(None);

    // Message picker state
    let (show_msg_picker, set_show_msg_picker) = signal(false);
    let (user_messages, set_user_messages) = signal::<Vec<WatcherMessageEntry>>(Vec::new());
    let (loading_msgs, set_loading_msgs) = signal(false);

    // Load sessions on mount
    let set_sessions_clone = set_sessions;
    let set_loading_clone = set_loading;
    let set_selected_id_clone = set_selected_id;
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match watchers::get_watcher_sessions().await {
                Ok(sess) => {
                    // Auto-select current session
                    let current_id = sess.iter()
                        .find(|s| s.is_current)
                        .map(|s| s.session_id.clone());
                    set_sessions_clone.set(sess);
                    if let Some(id) = current_id {
                        set_selected_id_clone.set(Some(id));
                    }
                }
                Err(e) => {
                    log::error!("WatcherModal: failed to load sessions: {}", e);
                    set_error.set(Some("Failed to load sessions".to_string()));
                }
            }
            set_loading_clone.set(false);
        });
    });

    // Load watcher config when selected_id changes
    Effect::new(move |_| {
        let sid = selected_id.get();
        if let Some(sid) = sid {
            set_error.set(None);
            leptos::task::spawn_local(async move {
                match watchers::get_watcher(&sid).await {
                    Ok(config) => {
                        set_continuation_msg.set(config.continuation_message.clone());
                        set_idle_timeout.set(config.idle_timeout_secs);
                        set_include_original.set(config.include_original);
                        set_original_message.set(config.original_message.clone());
                        set_hang_message.set(config.hang_message.clone());
                        set_hang_timeout.set(config.hang_timeout_secs);
                        set_existing_watcher.set(Some(config));
                    }
                    Err(_) => {
                        // No existing watcher — reset form to defaults
                        set_continuation_msg.set("Continue.".to_string());
                        set_idle_timeout.set(10);
                        set_include_original.set(false);
                        set_original_message.set(None);
                        set_hang_message.set("The previous attempt appears to have stalled. Please retry the task.".to_string());
                        set_hang_timeout.set(180);
                        set_existing_watcher.set(None);
                    }
                }
            });
        }
    });

    // Load messages for picker
    let load_messages = move || {
        let sid = selected_id.get_untracked();
        if let Some(sid) = sid {
            set_loading_msgs.set(true);
            set_show_msg_picker.set(true);
            leptos::task::spawn_local(async move {
                match watchers::get_watcher_messages(&sid).await {
                    Ok(msgs) => set_user_messages.set(msgs),
                    Err(_) => set_user_messages.set(Vec::new()),
                }
                set_loading_msgs.set(false);
            });
        }
    };

    // Save handler
    let handle_save = move || {
        let sid = match selected_id.get_untracked() {
            Some(s) => s,
            None => return,
        };
        set_saving.set(true);
        set_error.set(None);
        let msg = continuation_msg.get_untracked();
        let timeout = idle_timeout.get_untracked();
        let incl = include_original.get_untracked();
        let orig = original_message.get_untracked();
        let hang_msg = hang_message.get_untracked();
        let hang_to = hang_timeout.get_untracked();
        leptos::task::spawn_local(async move {
            let req = WatcherConfigRequest {
                session_id: sid,
                idle_timeout_secs: timeout,
                continuation_message: msg,
                include_original: incl,
                original_message: orig,
                hang_message: hang_msg,
                hang_timeout_secs: hang_to,
            };
            match watchers::create_watcher(&req).await {
                Ok(config) => {
                    set_existing_watcher.set(Some(config));
                    // Refresh sessions
                    if let Ok(sess) = watchers::get_watcher_sessions().await {
                        set_sessions.set(sess);
                    }
                }
                Err(e) => set_error.set(Some(e.message)),
            }
            set_saving.set(false);
        });
    };

    // Delete handler
    let handle_delete = move || {
        let sid = match selected_id.get_untracked() {
            Some(s) => s,
            None => return,
        };
        set_saving.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match watchers::delete_watcher(&sid).await {
                Ok(()) => {
                    set_existing_watcher.set(None);
                    set_continuation_msg.set("Continue.".to_string());
                    set_idle_timeout.set(10);
                    set_include_original.set(false);
                    set_original_message.set(None);
                    set_hang_message.set("The previous attempt appears to have stalled. Please retry the task.".to_string());
                    set_hang_timeout.set(180);
                    if let Ok(sess) = watchers::get_watcher_sessions().await {
                        set_sessions.set(sess);
                    }
                }
                Err(e) => set_error.set(Some(e.message)),
            }
            set_saving.set(false);
        });
    };

    let on_close_click = move |_: leptos::ev::MouseEvent| {
        on_close.run(());
    };

    // Status badge
    let status_badge = Memo::new(move |_| {
        let exists = existing_watcher.get();
        match exists {
            Some(w) => {
                let cls = match w.status.as_str() {
                    "active" | "watching" => "watcher-dot watcher-dot-green",
                    "countdown" => "watcher-dot watcher-dot-yellow",
                    _ => "watcher-dot watcher-dot-muted",
                };
                (cls, w.status.clone())
            }
            None => ("", String::new()),
        }
    });

    let has_selection = Memo::new(move |_| selected_id.get().is_some());
    let is_update = Memo::new(move |_| existing_watcher.get().is_some());

    view! {
        <ModalOverlay on_close=on_close class="watcher-modal">
                // Header
                <div class="watcher-modal-header">
                    <svg class="w-4 h-4 opacity-70" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                        <circle cx="12" cy="12" r="3"/>
                    </svg>
                    <span class="font-semibold text-sm">"Session Watcher"</span>
                    {move || {
                        let (cls, label) = status_badge.get();
                        if !label.is_empty() {
                            Some(view! {
                                <span class="watcher-status-badge">
                                    <span class=cls></span>
                                    {label}
                                </span>
                            })
                        } else {
                            None
                        }
                    }}
                    <div class="flex-1"/>
                    <button class="watcher-modal-close" on:click=on_close_click title="Close">
                        <IconX size=16 class="w-4 h-4" />
                    </button>
                </div>

                // Body
                <div class="watcher-modal-body">
                    // Session list (left panel)
                    <div class="watcher-session-list">
                        {move || {
                            if loading.get() {
                                view! {
                                    <div class="watcher-empty">
                                         <IconLoader2 size=16 class="w-4 h-4 spinning" />
                                        "Loading sessions..."
                                    </div>
                                }.into_any()
                            } else {
                                let sess = sessions.get();
                                if sess.is_empty() {
                                    view! {
                                        <div class="watcher-empty">"No sessions available"</div>
                                    }.into_any()
                                } else {
                                    let groups = group_sessions(&sess);
                                    view! {
                                        <div>
                                            {groups.into_iter().map(|(group, items)| {
                                                let label = group.label();
                                                view! {
                                                    <div class="watcher-session-group">
                                                        <div class="watcher-group-label">{label}</div>
                                                        {items.into_iter().map(|s| {
                                                            let sid = s.session_id.clone();
                                                            let sid_click = sid.clone();
                                                            let title = s.title.clone();
                                                            let project = s.project_name.clone();
                                                            let has_watcher = s.has_watcher;
                                                            view! {
                                                                <button
                                                                    class=move || {
                                                                        let mut cls = "watcher-session-item".to_string();
                                                                        if selected_id.get().as_deref() == Some(&sid) {
                                                                            cls.push_str(" selected");
                                                                        }
                                                                        cls
                                                                    }
                                                                    on:click=move |_| set_selected_id.set(Some(sid_click.clone()))
                                                                >
                                                                    <span class="watcher-session-title">{title}</span>
                                                                    {if has_watcher {
                                                                        Some(view! {
                                                                            <svg class="watcher-session-icon w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                                                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                                                                                <circle cx="12" cy="12" r="3"/>
                                                                            </svg>
                                                                        })
                                                                    } else {
                                                                        None
                                                                    }}
                                                                    <span class="watcher-session-project">{project}</span>
                                                                </button>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            }
                        }}
                    </div>

                    // Config form (right panel)
                    <div class="watcher-config-form">
                        {move || {
                            if !has_selection.get() {
                                return view! {
                                    <div class="watcher-empty">"Select a session to configure the watcher"</div>
                                }.into_any();
                            }

                            view! {
                                <div class="flex flex-col gap-3.5">
                                    // Continuation message
                                    <div class="watcher-form-section">
                                        <label class="watcher-form-label">
                                            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                <circle cx="12" cy="12" r="10"/>
                                                <polyline points="12,6 12,12 16,14"/>
                                            </svg>
                                            "Continuation Message"
                                        </label>
                                        <textarea
                                            class="watcher-form-textarea"
                                            rows=3
                                            prop:value=move || continuation_msg.get()
                                            on:input=move |e| set_continuation_msg.set(event_target_value(&e))
                                        />
                                        <span class="watcher-form-hint">"Sent when the session goes idle"</span>
                                    </div>

                                    // Idle timeout
                                    <div class="watcher-form-section">
                                        <label class="watcher-form-label">"Idle Timeout (seconds)"</label>
                                        <input
                                            type="number"
                                            class="watcher-form-input"
                                            min="1"
                                            prop:value=move || idle_timeout.get().to_string()
                                            on:input=move |e| {
                                                let val = event_target_value(&e).parse::<u64>().unwrap_or(1);
                                                set_idle_timeout.set(val.max(1));
                                            }
                                        />
                                    </div>

                                    // Include original message
                                    <div class="watcher-form-section">
                                        <label class="watcher-form-check">
                                            <input
                                                type="checkbox"
                                                prop:checked=move || include_original.get()
                                                on:change=move |e| {
                                                    set_include_original.set(event_target_checked(&e));
                                                }
                                            />
                                            "Include original user message"
                                        </label>
                                    </div>

                                    // Original message section
                                    {move || {
                                        if !include_original.get() {
                                            return None;
                                        }
                                        Some(view! {
                                            <div class="watcher-original-msg">
                                                {move || {
                                                    let orig = original_message.get();
                                                    match orig {
                                                        Some(text) => view! {
                                                            <div class="watcher-original-preview">
                                                                <span class="watcher-original-text">{text}</span>
                                                                <button
                                                                    class="watcher-btn-sm"
                                                                    on:click=move |_| set_original_message.set(None)
                                                                >"Clear"</button>
                                                            </div>
                                                        }.into_any(),
                                                        None => view! {
                                                            <button
                                                                class="watcher-btn-sm"
                                                                on:click=move |_| load_messages()
                                                            >"Pick from session messages"</button>
                                                        }.into_any(),
                                                    }
                                                }}

                                                // Message picker dropdown
                                                {move || {
                                                    if !show_msg_picker.get() {
                                                        return None;
                                                    }
                                                    Some(view! {
                                                        <div class="watcher-msg-picker">
                                                            {move || {
                                                                if loading_msgs.get() {
                                                                    return view! {
                                                                        <div class="watcher-msg-picker-empty">"Loading..."</div>
                                                                    }.into_any();
                                                                }
                                                                let msgs = user_messages.get();
                                                                if msgs.is_empty() {
                                                                    return view! {
                                                                        <div class="watcher-msg-picker-empty">"No messages found"</div>
                                                                    }.into_any();
                                                                }
                                                                view! {
                                                                    <div>
                                                                        {msgs.into_iter().map(|m| {
                                                                            let text = m.text.clone();
                                                                            let text_for_display = if text.chars().count() > 120 {
                                                                                let truncated: String = text.chars().take(120).collect();
                                                                                format!("{}...", truncated)
                                                                            } else {
                                                                                text.clone()
                                                                            };
                                                                            view! {
                                                                                <button
                                                                                    class="watcher-msg-picker-item"
                                                                                    on:click=move |_| {
                                                                                        set_original_message.set(Some(text.clone()));
                                                                                        set_show_msg_picker.set(false);
                                                                                    }
                                                                                >{text_for_display}</button>
                                                                            }
                                                                        }).collect_view()}
                                                                    </div>
                                                                }.into_any()
                                                            }}
                                                        </div>
                                                    })
                                                }}
                                            </div>
                                        })
                                    }}

                                    <div class="watcher-form-divider"/>

                                    // Hang detection message
                                    <div class="watcher-form-section">
                                        <label class="watcher-form-label">
                                            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                                                <line x1="12" y1="9" x2="12" y2="13"/>
                                                <line x1="12" y1="17" x2="12.01" y2="17"/>
                                            </svg>
                                            "Hang Detection Message"
                                        </label>
                                        <textarea
                                            class="watcher-form-textarea"
                                            rows=2
                                            prop:value=move || hang_message.get()
                                            on:input=move |e| set_hang_message.set(event_target_value(&e))
                                        />
                                        <span class="watcher-form-hint">"Sent when the session appears stalled"</span>
                                    </div>

                                    // Hang timeout
                                    <div class="watcher-form-section">
                                        <label class="watcher-form-label">"Hang Timeout (seconds)"</label>
                                        <input
                                            type="number"
                                            class="watcher-form-input"
                                            min="10"
                                            prop:value=move || hang_timeout.get().to_string()
                                            on:input=move |e| {
                                                let val = event_target_value(&e).parse::<u64>().unwrap_or(10);
                                                set_hang_timeout.set(val.max(10));
                                            }
                                        />
                                    </div>

                                    // Error
                                    {move || {
                                        error.get().map(|e| view! {
                                            <div class="watcher-form-error">{e}</div>
                                        })
                                    }}

                                    // Actions
                                    <div class="watcher-form-actions">
                                        <button
                                            class="watcher-btn watcher-btn-primary"
                                            disabled=move || saving.get()
                                            on:click=move |_| handle_save()
                                        >
                                            {move || if saving.get() {
                                                "Saving...".to_string()
                                            } else if is_update.get() {
                                                "Update Watcher".to_string()
                                            } else {
                                                "Start Watcher".to_string()
                                            }}
                                        </button>
                                        {move || {
                                            if is_update.get() {
                                                Some(view! {
                                                    <button
                                                        class="watcher-btn watcher-btn-danger"
                                                        disabled=move || saving.get()
                                                        on:click=move |_| handle_delete()
                                                    >"Remove"</button>
                                                })
                                            } else {
                                                None
                                            }
                                        }}
                                    </div>
                                </div>
                            }.into_any()
                        }}
                    </div>
                </div>

                // Footer
                <div class="watcher-modal-footer">
                    <kbd>"Esc"</kbd>
                    " Close"
                </div>
        </ModalOverlay>
    }
}

fn event_target_value(e: &leptos::ev::Event) -> String {
    use wasm_bindgen::JsCast;
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .or_else(|| {
            e.target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                .map(|el| el.value())
        })
        .unwrap_or_default()
}

fn event_target_checked(e: &leptos::ev::Event) -> bool {
    use wasm_bindgen::JsCast;
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}
