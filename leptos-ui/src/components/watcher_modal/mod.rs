//! WatcherModal — session watcher configuration.
//! Split: session list with keyboard nav (this file) + config form (config_form.rs).

mod config_form;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use crate::components::modal_overlay::ModalOverlay;
use crate::api::watchers;
use crate::types::api::{WatcherConfigResponse, WatcherSessionEntry};
use crate::components::icons::*;
use config_form::WatcherConfigForm;

// ── Session grouping ────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum SessionGroup { Current, Watched, Active, Other }

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

fn classify(s: &WatcherSessionEntry) -> SessionGroup {
    if s.is_current { SessionGroup::Current }
    else if s.has_watcher { SessionGroup::Watched }
    else if s.is_active { SessionGroup::Active }
    else { SessionGroup::Other }
}

fn group_sessions(sessions: &[WatcherSessionEntry]) -> Vec<(SessionGroup, Vec<WatcherSessionEntry>)> {
    let order = [SessionGroup::Current, SessionGroup::Watched, SessionGroup::Active, SessionGroup::Other];
    order.iter().filter_map(|g| {
        let items: Vec<_> = sessions.iter().filter(|s| classify(s) == *g).cloned().collect();
        if items.is_empty() { None } else { Some((g.clone(), items)) }
    }).collect()
}

/// Flatten grouped sessions into a single ordered list for keyboard nav.
fn flat_session_ids(sessions: &[WatcherSessionEntry]) -> Vec<String> {
    let groups = group_sessions(sessions);
    groups.into_iter().flat_map(|(_, items)| items.into_iter().map(|s| s.session_id)).collect()
}

// ── Component ───────────────────────────────────────────────────────

/// Mobile step: 0 = session list, 1 = config form.
/// Desktop ignores this — both panels are always visible via CSS.
#[component]
pub fn WatcherModal(on_close: Callback<()>) -> impl IntoView {
    let (sessions, set_sessions) = signal::<Vec<WatcherSessionEntry>>(Vec::new());
    let (selected_id, set_selected_id) = signal::<Option<String>>(None);
    let (loading, set_loading) = signal(true);
    let (mobile_step, set_mobile_step) = signal(1u8); // default: config (current pre-selected)

    // Load sessions on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match watchers::get_watcher_sessions().await {
                Ok(sess) => {
                    let current_id = sess.iter().find(|s| s.is_current).map(|s| s.session_id.clone());
                    set_sessions.set(sess);
                    if let Some(id) = current_id {
                        set_selected_id.set(Some(id));
                        set_mobile_step.set(1);
                    } else {
                        set_mobile_step.set(0);
                    }
                }
                Err(e) => {
                    log::error!("WatcherModal: failed to load sessions: {}", e);
                    set_mobile_step.set(0);
                }
            }
            set_loading.set(false);
        });
    });

    // Selecting a session on mobile → go to config step
    let select_session = move |sid: String| {
        set_selected_id.set(Some(sid));
        set_mobile_step.set(1);
    };

    // Keyboard navigation on session list
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" | "j" => {
                e.prevent_default();
                let flat = flat_session_ids(&sessions.get_untracked());
                if flat.is_empty() { return; }
                let cur = selected_id.get_untracked();
                let idx = cur.as_ref().and_then(|c| flat.iter().position(|s| s == c));
                let next = match idx {
                    Some(i) => (i + 1) % flat.len(),
                    None => 0,
                };
                set_selected_id.set(Some(flat[next].clone()));
            }
            "ArrowUp" | "k" => {
                e.prevent_default();
                let flat = flat_session_ids(&sessions.get_untracked());
                if flat.is_empty() { return; }
                let cur = selected_id.get_untracked();
                let idx = cur.as_ref().and_then(|c| flat.iter().position(|s| s == c));
                let next = match idx {
                    Some(0) | None => flat.len() - 1,
                    Some(i) => i - 1,
                };
                set_selected_id.set(Some(flat[next].clone()));
            }
            _ => {}
        }
    };

    // Scroll selected session into view
    Effect::new(move |_| {
        let sid = selected_id.get();
        if let (Some(sid), Some(doc)) = (sid, web_sys::window().and_then(|w| w.document())) {
            let sel = format!("[data-watcher-sid=\"{}\"]", sid);
            if let Ok(Some(el)) = doc.query_selector(&sel) {
                if let Some(html_el) = el.dyn_ref::<web_sys::HtmlElement>() {
                    html_el.scroll_into_view();
                }
            }
        }
    });

    let body_class = move || {
        let step = mobile_step.get();
        if step == 0 { "watcher-modal-body watcher-mobile-step-list" }
        else { "watcher-modal-body watcher-mobile-step-form" }
    };

    view! {
        <ModalOverlay on_close=on_close class="watcher-modal">
            <div class="watcher-modal-header">
                <svg class="w-4 h-4 opacity-70" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                    <circle cx="12" cy="12" r="3"/>
                </svg>
                <span class="font-semibold text-sm">"Session Watcher"</span>
                <div class="flex-1"/>
                <button class="watcher-modal-close" on:click=move |_| on_close.run(()) title="Close">
                    <IconX size=16 class="w-4 h-4" />
                </button>
            </div>

            <div class=body_class>
                // Session list panel
                <div class="watcher-session-list" on:keydown=on_keydown tabindex=0>
                    {move || {
                        if loading.get() {
                            return view! {
                                <div class="watcher-empty">
                                    <IconLoader2 size=16 class="w-4 h-4 spinning" />
                                    "Loading sessions..."
                                </div>
                            }.into_any();
                        }
                        let sess = sessions.get();
                        if sess.is_empty() {
                            return view! {
                                <div class="watcher-empty">"No sessions available"</div>
                            }.into_any();
                        }
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
                                                let sid_attr = sid.clone();
                                                let title = s.title.clone();
                                                let project = s.project_name.clone();
                                                let has_watcher = s.has_watcher;
                                                view! {
                                                    <button
                                                        class=move || {
                                                            let mut cls = "watcher-session-item".to_string();
                                                            if selected_id.get().as_deref() == Some(&sid) { cls.push_str(" selected"); }
                                                            cls
                                                        }
                                                        attr:data-watcher-sid=sid_attr
                                                        on:click=move |_| select_session(sid_click.clone())
                                                    >
                                                        <span class="watcher-session-title">{title}</span>
                                                        {has_watcher.then(|| view! {
                                                            <svg class="watcher-session-icon w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                                                                <circle cx="12" cy="12" r="3"/>
                                                            </svg>
                                                        })}
                                                        <span class="watcher-session-project">{project}</span>
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </div>

                // Config form panel (with mobile back button)
                <div class="watcher-config-panel">
                    <button
                        class="watcher-mobile-back"
                        on:click=move |_| set_mobile_step.set(0)
                    >
                        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <polyline points="15,18 9,12 15,6"/>
                        </svg>
                        "Sessions"
                    </button>
                    <WatcherConfigForm selected_id=selected_id set_sessions=set_sessions />
                </div>
            </div>

            <div class="watcher-modal-footer">
                <kbd>"Up/Down"</kbd>" Navigate "<kbd>"Esc"</kbd>" Close"
            </div>
        </ModalOverlay>
    }
}
