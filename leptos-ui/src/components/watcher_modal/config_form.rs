//! Watcher config form — right panel of the watcher modal.
//! Contains the continuation message, idle timeout, hang detection,
//! original message picker, and save/delete actions.

use leptos::prelude::*;
use crate::api::watchers::{self, WatcherConfigRequest, WatcherMessageEntry};
use crate::types::api::{WatcherConfigResponse, WatcherSessionEntry};
use crate::components::icons::*;

/// Props bundle for the watcher config form.
pub struct ConfigFormProps {
    pub selected_id: ReadSignal<Option<String>>,
    pub sessions: ReadSignal<Vec<WatcherSessionEntry>>,
    pub set_sessions: WriteSignal<Vec<WatcherSessionEntry>>,
}

#[component]
pub fn WatcherConfigForm(
    selected_id: ReadSignal<Option<String>>,
    set_sessions: WriteSignal<Vec<WatcherSessionEntry>>,
) -> impl IntoView {
    let (saving, set_saving) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    // Form state
    let (continuation_msg, set_continuation_msg) = signal("Continue.".to_string());
    let (idle_timeout, set_idle_timeout) = signal(10u64);
    let (include_original, set_include_original) = signal(false);
    let (original_message, set_original_message) = signal::<Option<String>>(None);
    let (hang_message, set_hang_message) = signal(
        "The previous attempt appears to have stalled. Please retry the task.".to_string(),
    );
    let (hang_timeout, set_hang_timeout) = signal(180u64);
    let (existing_watcher, set_existing_watcher) = signal::<Option<WatcherConfigResponse>>(None);

    // Message picker state
    let (show_msg_picker, set_show_msg_picker) = signal(false);
    let (user_messages, set_user_messages) = signal::<Vec<WatcherMessageEntry>>(Vec::new());
    let (loading_msgs, set_loading_msgs) = signal(false);

    // Load watcher config when selected_id changes
    Effect::new(move |_| {
        let Some(sid) = selected_id.get() else { return; };
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
                    set_continuation_msg.set("Continue.".to_string());
                    set_idle_timeout.set(10);
                    set_include_original.set(false);
                    set_original_message.set(None);
                    set_hang_message.set(
                        "The previous attempt appears to have stalled. Please retry the task."
                            .to_string(),
                    );
                    set_hang_timeout.set(180);
                    set_existing_watcher.set(None);
                }
            }
        });
    });

    // Load messages for picker
    let load_messages = move || {
        let Some(sid) = selected_id.get_untracked() else { return; };
        set_loading_msgs.set(true);
        set_show_msg_picker.set(true);
        leptos::task::spawn_local(async move {
            match watchers::get_watcher_messages(&sid).await {
                Ok(msgs) => set_user_messages.set(msgs),
                Err(_) => set_user_messages.set(Vec::new()),
            }
            set_loading_msgs.set(false);
        });
    };

    // Save
    let handle_save = move || {
        let Some(sid) = selected_id.get_untracked() else { return; };
        set_saving.set(true);
        set_error.set(None);
        let req = WatcherConfigRequest {
            session_id: sid,
            idle_timeout_secs: idle_timeout.get_untracked(),
            continuation_message: continuation_msg.get_untracked(),
            include_original: include_original.get_untracked(),
            original_message: original_message.get_untracked(),
            hang_message: hang_message.get_untracked(),
            hang_timeout_secs: hang_timeout.get_untracked(),
        };
        leptos::task::spawn_local(async move {
            match watchers::create_watcher(&req).await {
                Ok(config) => {
                    set_existing_watcher.set(Some(config));
                    if let Ok(sess) = watchers::get_watcher_sessions().await {
                        set_sessions.set(sess);
                    }
                }
                Err(e) => set_error.set(Some(e.message)),
            }
            set_saving.set(false);
        });
    };

    // Delete
    let handle_delete = move || {
        let Some(sid) = selected_id.get_untracked() else { return; };
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
                    set_hang_message.set(
                        "The previous attempt appears to have stalled. Please retry the task."
                            .to_string(),
                    );
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

    let has_selection = Memo::new(move |_| selected_id.get().is_some());
    let is_update = Memo::new(move |_| existing_watcher.get().is_some());

    view! {
        <div class="watcher-config-form">
            {move || {
                if !has_selection.get() {
                    return view! {
                        <div class="watcher-empty">"Select a session to configure the watcher"</div>
                    }.into_any();
                }
                view! {
                    <div class="flex flex-col gap-3.5">
                        <div class="watcher-form-section">
                            <label class="watcher-form-label">
                                <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <circle cx="12" cy="12" r="10"/><polyline points="12,6 12,12 16,14"/>
                                </svg>
                                "Continuation Message"
                            </label>
                            <textarea class="watcher-form-textarea" rows=3
                                prop:value=move || continuation_msg.get()
                                on:input=move |e| set_continuation_msg.set(ev_val(&e))
                            />
                            <span class="watcher-form-hint">"Sent when the session goes idle"</span>
                        </div>
                        <div class="watcher-form-section">
                            <label class="watcher-form-label">"Idle Timeout (seconds)"</label>
                            <input type="number" class="watcher-form-input" min="1"
                                prop:value=move || idle_timeout.get().to_string()
                                on:input=move |e| set_idle_timeout.set(ev_val(&e).parse::<u64>().unwrap_or(1).max(1))
                            />
                        </div>
                        <div class="watcher-form-section">
                            <label class="watcher-form-check">
                                <input type="checkbox" prop:checked=move || include_original.get()
                                    on:change=move |e| set_include_original.set(ev_checked(&e))
                                />" Include original user message"
                            </label>
                        </div>
                        {move || include_original.get().then(|| view! {
                            <div class="watcher-original-msg">
                                {move || match original_message.get() {
                                    Some(text) => view! {
                                        <div class="watcher-original-preview">
                                            <span class="watcher-original-text">{text}</span>
                                            <button class="watcher-btn-sm" on:click=move |_| set_original_message.set(None)>"Clear"</button>
                                        </div>
                                    }.into_any(),
                                    None => view! {
                                        <button class="watcher-btn-sm" on:click=move |_| load_messages()>"Pick from session messages"</button>
                                    }.into_any(),
                                }}
                                {move || show_msg_picker.get().then(|| view! {
                                    <div class="watcher-msg-picker">
                                        {move || {
                                            if loading_msgs.get() {
                                                return view! { <div class="watcher-msg-picker-empty">"Loading..."</div> }.into_any();
                                            }
                                            let msgs = user_messages.get();
                                            if msgs.is_empty() {
                                                return view! { <div class="watcher-msg-picker-empty">"No messages found"</div> }.into_any();
                                            }
                                            view! {
                                                <div>{msgs.into_iter().map(|m| {
                                                    let text = m.text.clone();
                                                    let display = if text.chars().count() > 120 {
                                                        format!("{}...", text.chars().take(120).collect::<String>())
                                                    } else { text.clone() };
                                                    view! {
                                                        <button class="watcher-msg-picker-item" on:click=move |_| {
                                                            set_original_message.set(Some(text.clone()));
                                                            set_show_msg_picker.set(false);
                                                        }>{display}</button>
                                                    }
                                                }).collect_view()}</div>
                                            }.into_any()
                                        }}
                                    </div>
                                })}
                            </div>
                        })}
                        <div class="watcher-form-divider"/>
                        <div class="watcher-form-section">
                            <label class="watcher-form-label">
                                <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                                    <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
                                </svg>
                                "Hang Detection Message"
                            </label>
                            <textarea class="watcher-form-textarea" rows=2
                                prop:value=move || hang_message.get()
                                on:input=move |e| set_hang_message.set(ev_val(&e))
                            />
                            <span class="watcher-form-hint">"Sent when the session appears stalled"</span>
                        </div>
                        <div class="watcher-form-section">
                            <label class="watcher-form-label">"Hang Timeout (seconds)"</label>
                            <input type="number" class="watcher-form-input" min="10"
                                prop:value=move || hang_timeout.get().to_string()
                                on:input=move |e| set_hang_timeout.set(ev_val(&e).parse::<u64>().unwrap_or(10).max(10))
                            />
                        </div>
                        {move || error.get().map(|e| view! { <div class="watcher-form-error">{e}</div> })}
                        <div class="watcher-form-actions">
                            <button class="watcher-btn watcher-btn-primary" disabled=move || saving.get()
                                on:click=move |_| handle_save()
                            >{move || if saving.get() { "Saving..." } else if is_update.get() { "Update Watcher" } else { "Start Watcher" }}</button>
                            {move || is_update.get().then(|| view! {
                                <button class="watcher-btn watcher-btn-danger" disabled=move || saving.get()
                                    on:click=move |_| handle_delete()
                                >"Remove"</button>
                            })}
                        </div>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

// ── Status badge ────────────────────────────────────────────────────

pub fn status_badge_view(
    existing_watcher: Memo<Option<WatcherConfigResponse>>,
) -> impl IntoView {
    // Unused for now — status is in mod.rs header
}

// ── Helpers ─────────────────────────────────────────────────────────

fn ev_val(e: &leptos::ev::Event) -> String {
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

fn ev_checked(e: &leptos::ev::Event) -> bool {
    use wasm_bindgen::JsCast;
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}
