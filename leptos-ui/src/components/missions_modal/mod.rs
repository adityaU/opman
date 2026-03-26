//! MissionsModal — CRUD for missions with state-machine actions.
//! Split: main list + keyboard nav (this file), item row (item_view.rs),
//! helpers + API bodies (helpers.rs).

mod helpers;
mod item_view;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use crate::api::client::{api_fetch, api_post};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{Mission, MissionsListResponse, PersonalMemoryItem, ProjectInfo};
use crate::components::icons::*;
use helpers::{format_state, state_color, CreateMissionBody, STATE_ORDER};
use item_view::MissionItemRow;

#[component]
pub fn MissionsModal(
    on_close: Callback<()>,
    projects: Vec<ProjectInfo>,
    active_project_index: usize,
    active_session_id: Option<String>,
    #[prop(optional)] active_memory_items: Option<Vec<PersonalMemoryItem>>,
) -> impl IntoView {
    let memory_items = active_memory_items.unwrap_or_default();

    let (missions, set_missions) = signal(Vec::<Mission>::new());
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (goal, set_goal) = signal(String::new());
    let (max_iterations, set_max_iterations) = signal(10_u32);
    let (expanded_id, set_expanded_id) = signal(Option::<String>::None);
    let (selected_idx, set_selected_idx) = signal(0usize);

    // Load on mount
    {
        leptos::task::spawn_local(async move {
            match api_fetch::<MissionsListResponse>("/missions").await {
                Ok(resp) => set_missions.set(resp.missions),
                Err(e) => leptos::logging::warn!("Failed to load missions: {}", e),
            }
            set_loading.set(false);
        });
    }

    let active_session_id_create = active_session_id.clone();
    let handle_create = move |_: web_sys::MouseEvent| {
        let g = goal.get_untracked();
        if g.trim().is_empty() {
            return;
        }
        let mi = max_iterations.get_untracked();
        let pi = active_project_index;
        let sid = active_session_id_create.clone();

        set_saving.set(true);
        leptos::task::spawn_local(async move {
            let body = CreateMissionBody {
                goal: g.trim().to_string(),
                session_id: sid,
                project_index: pi,
                max_iterations: mi,
            };
            match api_post::<Mission>("/missions", &body).await {
                Ok(created) => {
                    set_missions.update(|list| list.insert(0, created));
                    set_goal.set(String::new());
                    set_max_iterations.set(10);
                }
                Err(e) => leptos::logging::warn!("Failed to create mission: {}", e),
            }
            set_saving.set(false);
        });
    };

    let project_name = projects
        .get(active_project_index)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| format!("Project {}", active_project_index));
    let session_ctx = active_session_id
        .as_ref()
        .map(|sid| format!(" \u{2022} session {}", &sid[..sid.len().min(8)]))
        .unwrap_or_else(|| " \u{2022} no session linked".to_string());

    /// Flatten missions into display order for keyboard navigation.
    fn flatten_missions(all: &[Mission]) -> Vec<Mission> {
        STATE_ORDER
            .iter()
            .flat_map(|&s| all.iter().filter(move |m| m.state == s).cloned())
            .collect()
    }

    // Scroll selected into view
    Effect::new(move |_| {
        let idx = selected_idx.get();
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let sel = format!("[data-mission-idx=\"{}\"]", idx);
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
                let len = flatten_missions(&missions.get_untracked()).len();
                if len > 0 {
                    set_selected_idx.update(|i| *i = (*i + 1) % len);
                }
            }
            "ArrowUp" | "k" => {
                e.prevent_default();
                let len = flatten_missions(&missions.get_untracked()).len();
                if len > 0 {
                    set_selected_idx.update(|i| *i = if *i == 0 { len - 1 } else { *i - 1 });
                }
            }
            "Enter" => {
                // Toggle expand/collapse eval history
                let all = flatten_missions(&missions.get_untracked());
                let idx = selected_idx.get_untracked();
                if let Some(m) = all.get(idx) {
                    let mid = m.id.clone();
                    set_expanded_id.update(|eid| {
                        if eid.as_ref() == Some(&mid) {
                            *eid = None;
                        } else {
                            *eid = Some(mid);
                        }
                    });
                }
            }
            _ => {}
        }
    };

    view! {
        <ModalOverlay on_close=on_close class="missions-modal">
            <div class="missions-header">
                <div class="missions-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="10" />
                        <circle cx="12" cy="12" r="6" />
                        <circle cx="12" cy="12" r="2" />
                    </svg>
                    <h3>"Missions"</h3>
                    <span class="missions-count">{move || missions.get().len()}</span>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close missions">
                    <IconX size=16 />
                </button>
            </div>

            <div class="missions-scrollable" on:keydown=on_keydown tabindex=0>
                // Create form
                <div class="missions-create">
                    <textarea class="missions-textarea"
                        prop:value=move || goal.get()
                        on:input=move |ev| set_goal.set(event_target_value(&ev))
                        placeholder="Describe the goal for this mission..." rows="3" />
                    <div class="missions-create-grid">
                        <label class="missions-iterations-label">
                            "Max iterations:"
                            <input class="missions-input missions-iterations-input"
                                type="number" min="0" max="100"
                                prop:value=move || max_iterations.get().to_string()
                                on:input=move |ev| {
                                    let val: u32 = event_target_value(&ev).parse().unwrap_or(0);
                                    set_max_iterations.set(val);
                                } />
                            <span class="missions-iterations-hint">
                                {move || if max_iterations.get() == 0 { "(unlimited)" } else { "" }}
                            </span>
                        </label>
                    </div>
                    <div class="missions-create-footer">
                        <span class="missions-context">
                            {project_name.clone()}{session_ctx.clone()}
                        </span>
                        <button class="missions-create-btn"
                            on:click=handle_create
                            disabled=move || saving.get() || goal.get().trim().is_empty()
                        >
                            <IconPlus size=14 />
                            {move || if saving.get() { " Creating..." } else { " Create mission" }}
                        </button>
                    </div>
                </div>

                // Memory strip
                {(!memory_items.is_empty()).then(|| {
                    let chips = memory_items.iter().take(4).map(|item| {
                        let label = item.label.clone();
                        view! { <span class="assistant-memory-chip">{label}</span> }
                    }).collect::<Vec<_>>();
                    view! {
                        <div class="assistant-memory-strip assistant-memory-strip-missions">
                            <span class="assistant-memory-strip-label">"Guided by memory"</span>
                            {chips}
                        </div>
                    }
                })}

                // Missions list
                <div class="missions-body">
                    {move || {
                        if loading.get() {
                            return view! { <div class="missions-empty">"Loading missions..."</div> }.into_any();
                        }
                        let all = missions.get();
                        let flat = flatten_missions(&all);
                        if flat.is_empty() {
                            return view! {
                                <div class="missions-empty">
                                    "No missions yet. Create one to define a goal and let the session work toward it automatically."
                                </div>
                            }.into_any();
                        }
                        let sel = selected_idx.get();

                        let sections = STATE_ORDER.iter().filter_map(|&state| {
                            let state_missions: Vec<_> = all.iter()
                                .filter(|m| m.state == state).cloned().collect();
                            if state_missions.is_empty() { return None; }
                            let color = state_color(state);
                            let label = format!("{} ({})", format_state(state), state_missions.len());

                            let rows = state_missions.into_iter().map(|m| {
                                // Find flat index for this mission
                                let flat_idx = flat.iter().position(|fm| fm.id == m.id).unwrap_or(0);
                                view! {
                                    <MissionItemRow
                                        mission=m is_selected={flat_idx == sel} idx=flat_idx
                                        projects=projects.clone() set_missions=set_missions
                                        expanded_id=expanded_id set_expanded_id=set_expanded_id
                                        on_hover=Callback::new(move |i| set_selected_idx.set(i))
                                    />
                                }
                            }).collect::<Vec<_>>();

                            Some(view! {
                                <section class="missions-section">
                                    <div class="missions-section-title" style=format!("color: {}", color)>
                                        {label}
                                    </div>
                                    {rows}
                                </section>
                            })
                        }).collect::<Vec<_>>();

                        view! { <div>{sections}</div> }.into_any()
                    }}
                </div>
            </div>

            <div class="missions-footer">
                <kbd>"Up/Down"</kbd>" Navigate "<kbd>"Enter"</kbd>" Expand "<kbd>"Esc"</kbd>" Close"
            </div>
        </ModalOverlay>
    }
}
