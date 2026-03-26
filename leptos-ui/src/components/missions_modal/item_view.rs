//! MissionsModal item row — mission card with action buttons + eval history.

use super::helpers::{
    can_perform_action, format_relative_date, format_verdict, state_color, MissionActionBody,
};
use crate::api::client::{api_delete, api_post};
use crate::types::api::{Mission, ProjectInfo};
use leptos::prelude::*;

/// SVG paths for each action icon.
fn action_svg(action: &str) -> &'static str {
    match action {
        "start" => r#"<polygon points="5 3 19 12 5 21 5 3" />"#,
        "pause" => {
            r#"<rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" />"#
        }
        "resume" => {
            r#"<polyline points="1 4 1 10 7 10" /><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />"#
        }
        "cancel" => {
            r#"<circle cx="12" cy="12" r="10" /><line x1="4.93" y1="4.93" x2="19.07" y2="19.07" />"#
        }
        _ => "",
    }
}

#[component]
pub fn MissionItemRow(
    mission: Mission,
    is_selected: bool,
    idx: usize,
    projects: Vec<ProjectInfo>,
    set_missions: WriteSignal<Vec<Mission>>,
    expanded_id: ReadSignal<Option<String>>,
    set_expanded_id: WriteSignal<Option<String>>,
    on_hover: Callback<usize>,
) -> impl IntoView {
    let mid = mission.id.clone();
    let mid_expand = mission.id.clone();
    let mid_delete = mission.id.clone();
    let mission_state = mission.state.clone();
    let goal_text = mission.goal.clone();
    let proj_name = projects
        .get(mission.project_index)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| format!("Project {}", mission.project_index));
    let dot_color = state_color(&mission.state);
    let iter_text = format!(
        "iter {}/{}",
        mission.iteration,
        if mission.max_iterations == 0 {
            "\u{221e}".to_string()
        } else {
            mission.max_iterations.to_string()
        },
    );
    let verdict_text = mission
        .last_verdict
        .as_ref()
        .map(|v| format!("last: {}", format_verdict(v)));
    let session_text = if mission.session_id.is_empty() {
        "no session".to_string()
    } else {
        format!(
            "session {}",
            &mission.session_id[..mission.session_id.len().min(8)]
        )
    };
    let updated_text = format!("updated {}", format_relative_date(&mission.updated_at));
    let eval_summary = mission.last_eval_summary.clone();
    let eval_history = mission.eval_history.clone().unwrap_or_default();
    let has_eval_history = !eval_history.is_empty();
    let eval_history_len = eval_history.len();
    let item_class = if is_selected {
        format!("missions-item missions-item-{} selected", mission.state)
    } else {
        format!("missions-item missions-item-{}", mission.state)
    };

    // Build action buttons for valid state transitions
    let actions: Vec<(&str, String)> = ["start", "pause", "resume", "cancel"]
        .iter()
        .filter(|&&a| can_perform_action(&mission_state, a))
        .map(|&a| (a, mission.id.clone()))
        .collect();

    let action_buttons = actions
        .into_iter()
        .map(|(action, mid_a)| {
            let cls = format!("missions-action-btn missions-action-{}", action);
            let title = action.to_string();
            let svg_inner = action_svg(action).to_string();
            let action_str = action.to_string();
            view! {
                <button class=cls title=title
                    on:click=move |_| {
                        let mid_a = mid_a.clone();
                        let action_str = action_str.clone();
                        leptos::task::spawn_local(async move {
                            let path = format!("/missions/{}/action", mid_a);
                            let body = MissionActionBody { action: action_str };
                            match api_post::<Mission>(&path, &body).await {
                                Ok(updated) => set_missions.update(|list| {
                                    if let Some(m) = list.iter_mut().find(|m| m.id == updated.id) {
                                        *m = updated;
                                    }
                                }),
                                Err(e) => leptos::logging::warn!("Mission action failed: {}", e),
                            }
                        });
                    }
                >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
                        inner_html=svg_inner />
                </button>
            }
        })
        .collect::<Vec<_>>();

    view! {
        <div class=item_class attr:data-mission-idx=idx on:mouseenter=move |_| on_hover.run(idx)>
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
                        <button class="missions-item-expand"
                            on:click=move |_| {
                                set_expanded_id.update(|eid| {
                                    if eid.as_ref() == Some(&mid_exp) { *eid = None; }
                                    else { *eid = Some(mid_exp.clone()); }
                                });
                            }
                        >
                            {move || {
                                if expanded_id.get().as_ref() == Some(&mid_expand) {
                                    format!("Hide evaluation history ({})", eval_history_len)
                                } else {
                                    format!("Show evaluation history ({})", eval_history_len)
                                }
                            }}
                        </button>
                    }
                })}
                {move || {
                    let is_exp = expanded_id.get().as_ref() == Some(&mid);
                    if is_exp && !eval_history.is_empty() {
                        let records = eval_history.iter().map(|r| {
                            let iter_num = format!("#{}", r.iteration);
                            let vcls = format!("missions-eval-verdict missions-eval-verdict-{}", r.verdict);
                            let vlabel = format_verdict(&r.verdict);
                            let summary = r.summary.clone();
                            let next_step = r.next_step.clone();
                            view! {
                                <div class="missions-eval-record">
                                    <span class="missions-eval-iter">{iter_num}</span>
                                    <span class=vcls>{vlabel}</span>
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
                {action_buttons}
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
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="3 6 5 6 21 6" />
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                </button>
            </div>
        </div>
    }
}
