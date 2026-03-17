//! AssistantCenterModal — central dashboard for all assistant capabilities.
//! Matches React `AssistantCenterModal.tsx`.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_fetch, api_post};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{
    AssistantCenterStats, AssistantRecommendation, RecommendationsResponse,
    RoutineDefinition, RoutineRunRecord, RoutinesListResponse,
};

// ── API bodies ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct StatsBody {
    permissions: Vec<String>,
    questions: Vec<String>,
}

#[derive(Serialize)]
struct RecommendBody {
    permissions: Vec<String>,
    questions: Vec<String>,
}

#[derive(Serialize)]
struct RunRoutineBody {}

// ── Helpers ─────────────────────────────────────────────────────────

fn autonomy_label(mode: &str) -> &'static str {
    match mode {
        "observe" => "Observe",
        "nudge" => "Nudge",
        "continue" => "Continue",
        "autonomous" => "Autonomous",
        _ => "Unknown",
    }
}

fn priority_class(priority: &str) -> &'static str {
    match priority {
        "high" | "critical" => "assistant-center-recommendation assistant-center-recommendation-high",
        "medium" => "assistant-center-recommendation assistant-center-recommendation-medium",
        _ => "assistant-center-recommendation",
    }
}

fn trigger_label(trigger: &str) -> &'static str {
    match trigger {
        "scheduled" | "cron" => "scheduled",
        "manual" => "manual",
        _ => "other",
    }
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn AssistantCenterModal(
    on_close: Callback<()>,
    #[prop(optional)] on_open_inbox: Option<Callback<()>>,
    #[prop(optional)] on_open_missions: Option<Callback<()>>,
    #[prop(optional)] on_open_memory: Option<Callback<()>>,
    #[prop(optional)] on_open_autonomy: Option<Callback<()>>,
    #[prop(optional)] on_open_routines: Option<Callback<()>>,
    #[prop(optional)] on_open_delegation: Option<Callback<()>>,
    #[prop(optional)] on_open_workspaces: Option<Callback<()>>,
) -> impl IntoView {
    let (stats, set_stats) = signal(Option::<AssistantCenterStats>::None);
    let (recommendations, set_recommendations) = signal(Vec::<AssistantRecommendation>::new());
    let (quick_routines, set_quick_routines) = signal(Vec::<RoutineDefinition>::new());

    // Load stats + recommendations + routines on mount
    {
        leptos::task::spawn_local(async move {
            let stats_body = StatsBody { permissions: vec![], questions: vec![] };
            let recs_body = RecommendBody { permissions: vec![], questions: vec![] };

            let stats_fut = api_post::<AssistantCenterStats>("/assistant-center/stats", &stats_body);
            let recs_fut = api_post::<RecommendationsResponse>("/recommendations", &recs_body);
            let routines_fut = api_fetch::<RoutinesListResponse>("/routines");

            let (stats_res, recs_res, routines_res) = futures::join!(stats_fut, recs_fut, routines_fut);

            if let Ok(s) = stats_res {
                set_stats.set(Some(s));
            }
            if let Ok(r) = recs_res {
                set_recommendations.set(r.recommendations);
            }
            if let Ok(rl) = routines_res {
                let enabled: Vec<_> = rl.routines.into_iter().filter(|r| r.enabled).take(8).collect();
                set_quick_routines.set(enabled);
            }

        });
    }

    let handle_run_routine = Callback::new(move |rid: String| {
        leptos::task::spawn_local(async move {
            let path = format!("/routines/{}/run", rid);
            let body = RunRoutineBody {};
            match api_post::<RoutineRunRecord>(&path, &body).await {
                Ok(_) => leptos::logging::log!("Routine triggered"),
                Err(e) => leptos::logging::warn!("Failed to run routine: {}", e),
            }
        });
    });

    let card_cbs: [Option<Callback<()>>; 6] = [
        on_open_inbox,
        on_open_missions,
        on_open_memory,
        on_open_routines,
        on_open_delegation,
        on_open_workspaces,
    ];

    view! {
        <ModalOverlay on_close=on_close class="assistant-center-modal">
            // Header
            <div class="assistant-center-header">
                <div class="assistant-center-header-left">
                    <IconBot size=16 />
                    <h3>"Assistant Center"</h3>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close assistant center">
                    <IconX size=16 />
                </button>
            </div>

            // Hero
            {move || {
                let s = stats.get()?;
                let mode = s.autonomy_mode.clone();
                let needs = s.pending_permissions + s.pending_questions + s.paused_missions;
                let active = s.active_missions;
                Some(view! {
                    <div class="assistant-center-hero">
                        <div class="assistant-center-mode">
                            "Mode: " {autonomy_label(&mode)}
                        </div>
                        <div class="assistant-center-summary">
                            {format!("{} needs attention", needs)}
                            {format!(" \u{2022} {} active missions", active)}
                        </div>
                    </div>
                })
            }}

            // Recommendations
            {move || {
                let recs = recommendations.get();
                if recs.is_empty() {
                    return None;
                }
                let rec_views = recs.iter().map(|rec| {
                    let cls = priority_class(&rec.priority);
                    let title = rec.title.clone();
                    let rationale = rec.rationale.clone();
                    view! {
                        <button class=cls>
                            <span class="assistant-center-recommendation-title">{title}</span>
                            <span class="assistant-center-recommendation-desc">{rationale}</span>
                        </button>
                    }
                }).collect::<Vec<_>>();
                Some(view! {
                    <div class="assistant-center-recommendations">
                        <div class="assistant-center-briefing-title">"Recommended next"</div>
                        {rec_views}
                    </div>
                })
            }}

            // Dashboard grid
            <div class="assistant-center-grid">
                {card_view(stats, card_cbs[0], "Inbox", "\u{1F4E5}", "Permissions, questions, and blocked work", |s| s.pending_permissions + s.pending_questions)}
                {card_view(stats, card_cbs[1], "Missions", "\u{1F3AF}", "Active goals", |s| s.total_missions)}
                {card_view(stats, card_cbs[2], "Memory", "\u{1F9E0}", "Persistent preferences and working norms", |s| s.memory_items)}
                {card_view(stats, card_cbs[3], "Routines", "\u{23F0}", "Scheduled and event-driven routines", |s| s.active_routines)}
                {card_view(stats, card_cbs[4], "Delegation", "\u{1F4BC}", "Active delegated work items", |s| s.active_delegations)}
                {card_view(stats, card_cbs[5], "Workspaces", "\u{1F5C2}", "Intent-oriented workspace launches", |s| s.workspace_count)}
            </div>

            // Quick routines
            {move || {
                let qr = quick_routines.get();
                if qr.is_empty() {
                    return None;
                }
                let routine_btns = qr.iter().map(|r| {
                    let rid = r.id.clone();
                    let name = r.name.clone();
                    let trigger = r.trigger.clone();
                    view! {
                        <button
                            class="assistant-center-routine-btn"
                            on:click={
                                let rid_c = rid.clone();
                                move |_: web_sys::MouseEvent| handle_run_routine.run(rid_c.clone())
                            }
                        >
                            <span class="assistant-center-routine-icon">
                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <polygon points="5 3 19 12 5 21 5 3" />
                                </svg>
                            </span>
                            <span class="assistant-center-routine-name">{name}</span>
                            <span style="font-size:10px;font-weight:500;padding:1px 5px;border-radius:4px;background:rgba(128,128,128,0.12);color:var(--color-muted,#888);white-space:nowrap;flex-shrink:0">
                                {trigger_label(&trigger)}
                            </span>
                        </button>
                    }
                }).collect::<Vec<_>>();

                Some(view! {
                    <div class="assistant-center-quick-routines">
                        <div class="assistant-center-briefing-title">"Routines"</div>
                        <div class="assistant-center-routine-list">{routine_btns}</div>
                    </div>
                })
            }}

            // Footer actions
            <div class="assistant-center-footer-actions">
                {on_open_autonomy.map(|cb| view! {
                    <button on:click=move |_| cb.run(())>"Adjust autonomy"</button>
                })}
                {on_open_inbox.map(|cb| view! {
                    <button on:click=move |_| cb.run(())>"Open needs-you queue"</button>
                })}
            </div>
        </ModalOverlay>
    }
}

fn card_view(
    stats: ReadSignal<Option<AssistantCenterStats>>,
    cb: Option<Callback<()>>,
    title: &'static str,
    icon: &'static str,
    desc: &'static str,
    stat_fn: fn(&AssistantCenterStats) -> usize,
) -> impl IntoView {
    let val = move || {
        stats.get().map(|s| format!("{}", stat_fn(&s))).unwrap_or_else(|| "...".into())
    };
    view! {
        <button
            class="assistant-center-card"
            on:click=move |_| { if let Some(c) = cb { c.run(()); } }
        >
            <div class="assistant-center-card-top">
                <span class="assistant-center-card-icon">{icon}</span>
                <span class="assistant-center-card-value">{val}</span>
            </div>
            <div class="assistant-center-card-title">{title}</div>
            <div class="assistant-center-card-desc">{desc}</div>
        </button>
    }
}
