//! DelegationBoardModal — CRUD for delegated work items.
//! Split: create form + keyboard-navigable list (this file) + item rendering (item_view.rs).

mod item_view;

use leptos::prelude::*;
use serde::Serialize;
use wasm_bindgen::JsCast;
use crate::api::client::{api_fetch, api_post};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{DelegatedWorkItem, DelegatedWorkListResponse, Mission};
use crate::components::icons::*;
use item_view::DelegationItemRow;

#[derive(Serialize)]
struct CreateDelegatedWorkBody {
    title: String,
    assignee: String,
    scope: String,
    mission_id: Option<String>,
    session_id: Option<String>,
    subagent_session_id: Option<String>,
}

#[component]
pub fn DelegationBoardModal(
    on_close: Callback<()>,
    missions: Vec<Mission>,
    active_session_id: Option<String>,
    #[prop(optional)] on_open_session: Option<Callback<String>>,
) -> impl IntoView {
    let (items, set_items) = signal(Vec::<DelegatedWorkItem>::new());
    let (loading, set_loading) = signal(true);
    let (title, set_title) = signal(String::new());
    let (assignee, set_assignee) = signal("build".to_string());
    let (scope, set_scope) = signal(String::new());
    let (mission_id, set_mission_id) = signal(String::new());
    let (subagent_session_id, set_subagent_session_id) = signal(String::new());
    let (selected_idx, set_selected_idx) = signal(0usize);

    // Load on mount
    {
        leptos::task::spawn_local(async move {
            match api_fetch::<DelegatedWorkListResponse>("/delegation").await {
                Ok(resp) => set_items.set(resp.items),
                Err(e) => leptos::logging::warn!("Failed to load delegation board: {}", e),
            }
            set_loading.set(false);
        });
    }

    let active_session_id_create = active_session_id.clone();
    let handle_create = move |_: web_sys::MouseEvent| {
        let t = title.get_untracked();
        let s = scope.get_untracked();
        if t.trim().is_empty() || s.trim().is_empty() { return; }
        let a = assignee.get_untracked();
        let m = mission_id.get_untracked();
        let sub = subagent_session_id.get_untracked();
        let sid = active_session_id_create.clone();

        leptos::task::spawn_local(async move {
            let body = CreateDelegatedWorkBody {
                title: t.trim().to_string(),
                assignee: a.trim().to_string(),
                scope: s.trim().to_string(),
                mission_id: if m.is_empty() { None } else { Some(m) },
                session_id: sid,
                subagent_session_id: if sub.is_empty() { None } else { Some(sub) },
            };
            match api_post::<DelegatedWorkItem>("/delegation", &body).await {
                Ok(item) => {
                    set_items.update(|list| list.insert(0, item));
                    set_title.set(String::new());
                    set_scope.set(String::new());
                    set_mission_id.set(String::new());
                    set_subagent_session_id.set(String::new());
                }
                Err(e) => leptos::logging::warn!("Failed to create delegation: {}", e),
            }
        });
    };

    let active_session_display = active_session_id
        .as_ref()
        .map(|s| s.chars().take(8).collect::<String>())
        .unwrap_or_else(|| "none".to_string());

    // Scroll selected into view
    Effect::new(move |_| {
        let idx = selected_idx.get();
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let sel = format!("[data-deleg-idx=\"{}\"]", idx);
            if let Ok(Some(el)) = doc.query_selector(&sel) {
                if let Some(html_el) = el.dyn_ref::<web_sys::HtmlElement>() {
                    html_el.scroll_into_view();
                }
            }
        }
    });

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        match e.key().as_str() {
            "ArrowDown" | "j" => {
                e.prevent_default();
                let len = items.get_untracked().len();
                if len > 0 { set_selected_idx.update(|i| *i = (*i + 1) % len); }
            }
            "ArrowUp" | "k" => {
                e.prevent_default();
                let len = items.get_untracked().len();
                if len > 0 { set_selected_idx.update(|i| *i = if *i == 0 { len - 1 } else { *i - 1 }); }
            }
            _ => {}
        }
    };

    view! {
        <ModalOverlay on_close=on_close class="delegation-modal">
            <div class="delegation-header">
                <div class="delegation-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="2" y="7" width="20" height="14" rx="2" ry="2" />
                        <path d="M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16" />
                    </svg>
                    <h3>"Delegation Board"</h3>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close delegation board">
                    <IconX size=16 />
                </button>
            </div>

            <div class="delegation-scrollable" on:keydown=on_keydown tabindex=0>
                // Create form
                <div class="delegation-create">
                    <input class="delegation-input"
                        prop:value=move || title.get()
                        on:input=move |ev| set_title.set(event_target_value(&ev))
                        placeholder="Delegated task title" />
                    <input class="delegation-input"
                        prop:value=move || assignee.get()
                        on:input=move |ev| set_assignee.set(event_target_value(&ev))
                        placeholder="Assignee" />
                    <textarea class="delegation-textarea"
                        prop:value=move || scope.get()
                        on:input=move |ev| set_scope.set(event_target_value(&ev))
                        rows="3" placeholder="Bounded task scope" />
                    <div class="delegation-create-grid">
                        <select class="delegation-select"
                            prop:value=move || mission_id.get()
                            on:change=move |ev| set_mission_id.set(event_target_value(&ev))
                        >
                            <option value="">"No mission link"</option>
                            {missions.iter().map(|m| {
                                let id = m.id.clone();
                                let goal = m.goal.clone();
                                view! { <option value=id>{goal}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                        <input class="delegation-input"
                            prop:value=move || subagent_session_id.get()
                            on:input=move |ev| set_subagent_session_id.set(event_target_value(&ev))
                            placeholder="Subagent session ID (optional)" />
                    </div>
                    <div class="delegation-create-footer">
                        <span class="delegation-context">"Current session: " {active_session_display}</span>
                        <button class="delegation-create-btn" on:click=handle_create
                            disabled=move || title.get().trim().is_empty() || scope.get().trim().is_empty()
                        ><IconPlus size=14 />" Add delegated work"</button>
                    </div>
                </div>

                // Item list
                <div class="delegation-body">
                    {move || {
                        if loading.get() {
                            return view! { <div class="delegation-empty">"Loading delegation board..."</div> }.into_any();
                        }
                        let item_list = items.get();
                        if item_list.is_empty() {
                            return view! { <div class="delegation-empty">"No delegated work yet."</div> }.into_any();
                        }
                        let sel = selected_idx.get();
                        let open_cb = on_open_session;
                        view! {
                            <div>{item_list.into_iter().enumerate().map(|(idx, item)| {
                                let cb = open_cb;
                                view! {
                                    <DelegationItemRow
                                        item=item is_selected={idx == sel} idx=idx
                                        set_items=set_items
                                        on_hover=Callback::new(move |i| set_selected_idx.set(i))
                                        on_open_session=cb
                                    />
                                }
                            }).collect::<Vec<_>>()}</div>
                        }.into_any()
                    }}
                </div>
            </div>
            <div class="delegation-footer">
                <kbd>"Up/Down"</kbd>" Navigate "<kbd>"Esc"</kbd>" Close"
            </div>
        </ModalOverlay>
    }
}
