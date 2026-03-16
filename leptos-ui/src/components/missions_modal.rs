//! MissionsModal — CRUD for missions with state-machine actions.
//! Matches React `missions-modal/MissionsModal.tsx`.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_delete, api_fetch, api_post};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{Mission, MissionsListResponse, PersonalMemoryItem, ProjectInfo};
use crate::components::icons::*;

// ── Helpers ─────────────────────────────────────────────────────────

const STATE_ORDER: &[&str] = &[
    "executing", "evaluating", "pending", "paused", "completed", "failed", "cancelled",
];

fn format_state(state: &str) -> &'static str {
    match state {
        "pending" => "Pending",
        "executing" => "Executing",
        "evaluating" => "Evaluating",
        "paused" => "Paused",
        "completed" => "Completed",
        "cancelled" => "Cancelled",
        "failed" => "Failed",
        _ => "Unknown",
    }
}

fn state_color(state: &str) -> &'static str {
    match state {
        "executing" | "evaluating" => "var(--color-info, #5c8fff)",
        "paused" => "var(--color-warning, #e6a817)",
        "completed" => "var(--color-success, #4caf50)",
        "failed" | "cancelled" => "var(--color-error, #e05252)",
        _ => "var(--color-text-muted, #999)",
    }
}

fn format_verdict(verdict: &str) -> &'static str {
    match verdict {
        "achieved" => "Achieved",
        "continue" => "Continue",
        "blocked" => "Blocked",
        "failed" => "Failed",
        _ => "Unknown",
    }
}

fn format_relative_date(iso: &str) -> String {
    iso.chars().take(16).collect::<String>().replace('T', " ")
}

fn can_perform_action(state: &str, action: &str) -> bool {
    match action {
        "start" => state == "pending",
        "pause" => state == "executing" || state == "evaluating",
        "resume" => state == "paused",
        "cancel" => matches!(state, "pending" | "executing" | "evaluating" | "paused"),
        _ => false,
    }
}

// ── API bodies ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateMissionBody {
    goal: String,
    session_id: Option<String>,
    project_index: usize,
    max_iterations: u32,
}

#[derive(Serialize)]
struct MissionActionBody {
    action: String,
}

#[derive(Serialize)]
struct UpdateMissionBody {
    goal: String,
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn MissionsModal(
    on_close: Callback<()>,
    projects: Vec<ProjectInfo>,
    active_project_index: usize,
    active_session_id: Option<String>,
    #[prop(optional)]
    active_memory_items: Option<Vec<PersonalMemoryItem>>,
) -> impl IntoView {
    let memory_items = active_memory_items.unwrap_or_default();

    let (missions, set_missions) = signal(Vec::<Mission>::new());
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (goal, set_goal) = signal(String::new());
    let (max_iterations, set_max_iterations) = signal(10_u32);
    let (expanded_id, set_expanded_id) = signal(Option::<String>::None);

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
    let active_session_id_ctx = active_session_id;

    let session_ctx = active_session_id_ctx
        .as_ref()
        .map(|sid| format!(" \u{2022} session {}", &sid[..sid.len().min(8)]))
        .unwrap_or_else(|| " \u{2022} no session linked".to_string());

    view! {
        <ModalOverlay on_close=on_close class="missions-modal">
            <div class="missions-header">
                <div class="missions-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
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

            <div class="missions-scrollable">
                // Create form
                <div class="missions-create">
                    <textarea
                        class="missions-textarea"
                        prop:value=move || goal.get()
                        on:input=move |ev| set_goal.set(event_target_value(&ev))
                        placeholder="Describe the goal for this mission..."
                        rows="3"
                    />
                    <div class="missions-create-grid">
                        <label class="missions-iterations-label">
                            "Max iterations:"
                            <input
                                class="missions-input missions-iterations-input"
                                type="number"
                                min="0"
                                max="100"
                                prop:value=move || max_iterations.get().to_string()
                                on:input=move |ev| {
                                    let val: u32 = event_target_value(&ev).parse().unwrap_or(0);
                                    set_max_iterations.set(val);
                                }
                            />
                            <span class="missions-iterations-hint">
                                {move || if max_iterations.get() == 0 { "(unlimited)" } else { "" }}
                            </span>
                        </label>
                    </div>
                    <div class="missions-create-footer">
                        <span class="missions-context">
                            {project_name.clone()}{session_ctx.clone()}
                        </span>
                        <button
                            class="missions-create-btn"
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
                        if all.is_empty() {
                            return view! {
                                <div class="missions-empty">
                                    "No missions yet. Create one to define a goal and let the session work toward it automatically."
                                </div>
                            }.into_any();
                        }

                        let sections = STATE_ORDER.iter().filter_map(|&state| {
                            let state_missions: Vec<_> = all.iter().filter(|m| m.state == state).cloned().collect();
                            if state_missions.is_empty() {
                                return None;
                            }
                            let color = state_color(state);
                            let label = format!("{} ({})", format_state(state), state_missions.len());

                            let items_views = state_missions.into_iter().map(|mission| {
                                let mid = mission.id.clone();
                                let mid_expand = mission.id.clone();
                                let mid_delete = mission.id.clone();
                                let mission_state = mission.state.clone();
                                let goal_text = mission.goal.clone();
                                let proj_name = projects.get(mission.project_index).map(|p| p.name.clone()).unwrap_or_else(|| format!("Project {}", mission.project_index));
                                let dot_color = state_color(&mission.state);

                                let iter_text = format!(
                                    "iter {}/{}",
                                    mission.iteration,
                                    if mission.max_iterations == 0 { "\u{221e}".to_string() } else { mission.max_iterations.to_string() }
                                );

                                let verdict_text = mission.last_verdict.as_ref().map(|v| format!("last: {}", format_verdict(v)));
                                let session_text = if mission.session_id.is_empty() {
                                    "no session".to_string()
                                } else {
                                    format!("session {}", &mission.session_id[..mission.session_id.len().min(8)])
                                };
                                let updated_text = format!("updated {}", format_relative_date(&mission.updated_at));
                                let eval_summary = mission.last_eval_summary.clone();
                                let eval_history = mission.eval_history.clone().unwrap_or_default();
                                let has_eval_history = !eval_history.is_empty();
                                let eval_history_len = eval_history.len();
                                let mission_state_for_actions = mission_state.clone();
                                let item_class = format!("missions-item missions-item-{}", mission.state);

                                view! {
                                    <div class=item_class>
                                        <div class="missions-item-main">
                                            <div class="missions-item-row">
                                                <span class="missions-item-state-dot" style=format!("background: {}", dot_color) />
                                                <span class="missions-item-goal">{goal_text}</span>
                                                <span class="missions-item-project">{proj_name}</span>
                                            </div>
                                            <div class="missions-item-meta">
                                                <span>{iter_text}</span>
                                                {verdict_text.map(|vt| view! { <span>{vt}</span> })}
                                                <span>{session_text}</span>
                                                <span>{updated_text}</span>
                                            </div>
                                            {eval_summary.map(|es| view! {
                                                <div class="missions-item-eval-summary">{es}</div>
                                            })}
                                            {has_eval_history.then(|| {
                                                let mid_exp = mid_expand.clone();
                                                view! {
                                                    <button
                                                        class="missions-item-expand"
                                                        on:click=move |_| {
                                                            set_expanded_id.update(|eid| {
                                                                if eid.as_ref() == Some(&mid_exp) {
                                                                    *eid = None;
                                                                } else {
                                                                    *eid = Some(mid_exp.clone());
                                                                }
                                                            });
                                                        }
                                                    >
                                                        {move || {
                                                            let is_expanded = expanded_id.get().as_ref() == Some(&mid_expand);
                                                            if is_expanded { format!("Hide evaluation history ({})", eval_history_len) }
                                                            else { format!("Show evaluation history ({})", eval_history_len) }
                                                        }}
                                                    </button>
                                                }
                                            })}
                                            {move || {
                                                let is_expanded = expanded_id.get().as_ref() == Some(&mid);
                                                if is_expanded && !eval_history.is_empty() {
                                                    let records = eval_history.iter().map(|record| {
                                                        let iter_num = format!("#{}", record.iteration);
                                                        let verdict_cls = format!("missions-eval-verdict missions-eval-verdict-{}", record.verdict);
                                                        let verdict_label = format_verdict(&record.verdict);
                                                        let summary = record.summary.clone();
                                                        let next_step = record.next_step.clone();
                                                        view! {
                                                            <div class="missions-eval-record">
                                                                <span class="missions-eval-iter">{iter_num}</span>
                                                                <span class=verdict_cls>{verdict_label}</span>
                                                                <span class="missions-eval-summary">{summary}</span>
                                                                {next_step.map(|ns| view! {
                                                                    <span class="missions-eval-next">"Next: " {ns}</span>
                                                                })}
                                                            </div>
                                                        }
                                                    }).collect::<Vec<_>>();
                                                    Some(view! { <div class="missions-eval-history">{records}</div> })
                                                } else {
                                                    None
                                                }
                                            }}
                                        </div>
                                        <div class="missions-item-actions">
                                            // Action buttons based on state
                                            {can_perform_action(&mission_state_for_actions, "start").then(|| {
                                                let mid_a = mid_delete.clone();
                                                view! {
                                                    <button class="missions-action-btn missions-action-start" title="Start"
                                                        on:click=move |_| {
                                                            let mid_a = mid_a.clone();
                                                            leptos::task::spawn_local(async move {
                                                                let path = format!("/missions/{}/action", mid_a);
                                                                let body = MissionActionBody { action: "start".to_string() };
                                                                match api_post::<Mission>(&path, &body).await {
                                                                    Ok(updated) => set_missions.update(|list| {
                                                                        if let Some(m) = list.iter_mut().find(|m| m.id == updated.id) { *m = updated; }
                                                                    }),
                                                                    Err(e) => leptos::logging::warn!("Mission action failed: {}", e),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <polygon points="5 3 19 12 5 21 5 3" />
                                                        </svg>
                                                    </button>
                                                }
                                            })}
                                            {can_perform_action(&mission_state_for_actions, "pause").then(|| {
                                                let mid_a = mid_delete.clone();
                                                view! {
                                                    <button class="missions-action-btn missions-action-pause" title="Pause"
                                                        on:click=move |_| {
                                                            let mid_a = mid_a.clone();
                                                            leptos::task::spawn_local(async move {
                                                                let path = format!("/missions/{}/action", mid_a);
                                                                let body = MissionActionBody { action: "pause".to_string() };
                                                                match api_post::<Mission>(&path, &body).await {
                                                                    Ok(updated) => set_missions.update(|list| {
                                                                        if let Some(m) = list.iter_mut().find(|m| m.id == updated.id) { *m = updated; }
                                                                    }),
                                                                    Err(e) => leptos::logging::warn!("Mission action failed: {}", e),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <rect x="6" y="4" width="4" height="16" />
                                                            <rect x="14" y="4" width="4" height="16" />
                                                        </svg>
                                                    </button>
                                                }
                                            })}
                                            {can_perform_action(&mission_state_for_actions, "resume").then(|| {
                                                let mid_a = mid_delete.clone();
                                                view! {
                                                    <button class="missions-action-btn missions-action-resume" title="Resume"
                                                        on:click=move |_| {
                                                            let mid_a = mid_a.clone();
                                                            leptos::task::spawn_local(async move {
                                                                let path = format!("/missions/{}/action", mid_a);
                                                                let body = MissionActionBody { action: "resume".to_string() };
                                                                match api_post::<Mission>(&path, &body).await {
                                                                    Ok(updated) => set_missions.update(|list| {
                                                                        if let Some(m) = list.iter_mut().find(|m| m.id == updated.id) { *m = updated; }
                                                                    }),
                                                                    Err(e) => leptos::logging::warn!("Mission action failed: {}", e),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <polyline points="1 4 1 10 7 10" />
                                                            <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
                                                        </svg>
                                                    </button>
                                                }
                                            })}
                                            {can_perform_action(&mission_state_for_actions, "cancel").then(|| {
                                                let mid_a = mid_delete.clone();
                                                view! {
                                                    <button class="missions-action-btn missions-action-cancel" title="Cancel"
                                                        on:click=move |_| {
                                                            let mid_a = mid_a.clone();
                                                            leptos::task::spawn_local(async move {
                                                                let path = format!("/missions/{}/action", mid_a);
                                                                let body = MissionActionBody { action: "cancel".to_string() };
                                                                match api_post::<Mission>(&path, &body).await {
                                                                    Ok(updated) => set_missions.update(|list| {
                                                                        if let Some(m) = list.iter_mut().find(|m| m.id == updated.id) { *m = updated; }
                                                                    }),
                                                                    Err(e) => leptos::logging::warn!("Mission action failed: {}", e),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <circle cx="12" cy="12" r="10" />
                                                            <line x1="4.93" y1="4.93" x2="19.07" y2="19.07" />
                                                        </svg>
                                                    </button>
                                                }
                                            })}
                                            <button class="missions-delete-btn" aria-label="Delete mission"
                                                on:click=move |_| {
                                                    let del_id = mid_delete.clone();
                                                    leptos::task::spawn_local(async move {
                                                        let path = format!("/missions/{}", del_id);
                                                        if api_delete(&path).await.is_ok() {
                                                            set_missions.update(|list| list.retain(|m| m.id != del_id));
                                                        }
                                                    });
                                                }
                                            >
                                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <polyline points="3 6 5 6 21 6" />
                                                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                                                </svg>
                                            </button>
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>();

                            Some(view! {
                                <section class="missions-section">
                                    <div class="missions-section-title" style=format!("color: {}", color)>
                                        {label}
                                    </div>
                                    {items_views}
                                </section>
                            })
                        }).collect::<Vec<_>>();

                        view! { <div>{sections}</div> }.into_any()
                    }}
                </div>
            </div>
        </ModalOverlay>
    }
}
