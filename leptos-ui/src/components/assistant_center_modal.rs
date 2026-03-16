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

fn autonomy_color(mode: &str) -> &'static str {
    match mode {
        "observe" => "var(--color-text-muted, #999)",
        "nudge" => "var(--color-info, #5c8fff)",
        "continue" => "var(--color-warning, #e6a817)",
        "autonomous" => "var(--color-success, #4caf50)",
        _ => "var(--color-text-muted, #999)",
    }
}

fn priority_color(priority: &str) -> &'static str {
    match priority {
        "high" | "critical" => "var(--color-error, #e05252)",
        "medium" => "var(--color-warning, #e6a817)",
        _ => "var(--color-info, #5c8fff)",
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
    let (loading, set_loading) = signal(true);

    // Load stats + recommendations + routines on mount
    {
        leptos::task::spawn_local(async move {
            let stats_body = StatsBody { permissions: vec![], questions: vec![] };
            let recs_body = RecommendBody { permissions: vec![], questions: vec![] };

            // Fire all three fetches
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
            set_loading.set(false);
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

    // Dashboard card data
    struct DashCard {
        label: &'static str,
        icon: &'static str,
        description: &'static str,
    }

    let cards = [
        DashCard { label: "Inbox", icon: "\u{1F4E5}", description: "Items needing attention" },
        DashCard { label: "Missions", icon: "\u{1F3AF}", description: "Active goals" },
        DashCard { label: "Memory", icon: "\u{1F4AD}", description: "Stored preferences" },
        DashCard { label: "Routines", icon: "\u{23F0}", description: "Automated tasks" },
        DashCard { label: "Delegation", icon: "\u{1F91D}", description: "Assigned work" },
        DashCard { label: "Workspaces", icon: "\u{1F4BB}", description: "Saved layouts" },
    ];

    let card_callbacks: Vec<Option<Callback<()>>> = vec![
        on_open_inbox,
        on_open_missions,
        on_open_memory,
        on_open_routines,
        on_open_delegation,
        on_open_workspaces,
    ];

    view! {
        <ModalOverlay on_close=on_close class="assistant-center-modal">
            <div class="ac-header">
                <div class="ac-header-left">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="3" />
                        <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
                    </svg>
                    <h3>"Assistant Center"</h3>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close assistant center">
                    <IconX size=16 />
                </button>
            </div>

            <div class="ac-scrollable">
                {move || {
                    if loading.get() {
                        return view! { <div class="ac-empty">"Loading assistant center..."</div> }.into_any();
                    }

                    view! {
                        <div class="ac-content">
                            // Hero section
                            {stats.get().map(|s| {
                                let mode = s.autonomy_mode.clone();
                                let needs = s.pending_permissions + s.pending_questions;
                                let active = s.active_missions;
                                view! {
                                    <div class="ac-hero">
                                        <div class="ac-hero-autonomy" style=format!("color: {}", autonomy_color(&mode))>
                                            {autonomy_label(&mode)}
                                        </div>
                                        <div class="ac-hero-stats">
                                            <span class="ac-hero-stat">
                                                <strong>{needs}</strong>" needs attention"
                                            </span>
                                            <span class="ac-hero-stat">
                                                <strong>{active}</strong>" active missions"
                                            </span>
                                        </div>
                                    </div>
                                }
                            })}

                            // Recommendations
                            {move || {
                                let recs = recommendations.get();
                                if recs.is_empty() {
                                    return None;
                                }
                                let rec_views = recs.iter().map(|rec| {
                                    let color = priority_color(&rec.priority);
                                    let title = rec.title.clone();
                                    let rationale = rec.rationale.clone();
                                    view! {
                                        <div class="ac-recommendation" style=format!("border-left: 3px solid {}", color)>
                                            <div class="ac-rec-title">{title}</div>
                                            <div class="ac-rec-rationale">{rationale}</div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>();
                                Some(view! {
                                    <section class="ac-section">
                                        <div class="ac-section-title">"Recommendations"</div>
                                        {rec_views}
                                    </section>
                                })
                            }}
                        </div>
                    }.into_any()
                }}

                // Dashboard grid (always rendered)
                <div class="ac-dashboard-grid">
                    {cards.iter().enumerate().map(|(i, card)| {
                        let label = card.label;
                        let icon = card.icon;
                        let desc = card.description;
                        let cb = card_callbacks.get(i).and_then(|c| *c);
                        let stat_value = move || {
                            stats.get().map(|s| match label {
                                "Inbox" => s.pending_permissions + s.pending_questions,
                                "Missions" => s.total_missions,
                                "Memory" => s.memory_items,
                                "Routines" => s.active_routines,
                                "Delegation" => s.active_delegations,
                                "Workspaces" => s.workspace_count,
                                _ => 0,
                            }).unwrap_or(0)
                        };

                        view! {
                            <button
                                class="ac-card"
                                on:click=move |_| { if let Some(c) = cb { c.run(()); } }
                            >
                                <span class="ac-card-icon">{icon}</span>
                                <span class="ac-card-value">{stat_value}</span>
                                <span class="ac-card-label">{label}</span>
                                <span class="ac-card-desc">{desc}</span>
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                // Quick routines
                {move || {
                    let qr = quick_routines.get();
                    if qr.is_empty() {
                        return None;
                    }
                    let routine_chips = qr.iter().map(|r| {
                        let rid = r.id.clone();
                        let name = r.name.clone();
                        let trigger = r.trigger.clone();
                        view! {
                            <div class="ac-quick-routine">
                                <span class="ac-quick-routine-name">{name}</span>
                                <span class="ac-quick-routine-trigger">{trigger_label_short(&trigger)}</span>
                                <button
                                    class="ac-quick-routine-run"
                                    on:click={
                                        let rid_c = rid.clone();
                                        move |_: web_sys::MouseEvent| handle_run_routine.run(rid_c.clone())
                                    }
                                >
                                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <polygon points="5 3 19 12 5 21 5 3" />
                                    </svg>
                                </button>
                            </div>
                        }
                    }).collect::<Vec<_>>();

                    Some(view! {
                        <section class="ac-section">
                            <div class="ac-section-title">"Quick Routines"</div>
                            <div class="ac-quick-routines">{routine_chips}</div>
                        </section>
                    })
                }}

                // Footer actions
                <div class="ac-footer">
                    {on_open_autonomy.map(|cb| view! {
                        <button class="ac-footer-btn" on:click=move |_| cb.run(())>
                            "Adjust Autonomy"
                        </button>
                    })}
                    {on_open_inbox.map(|cb| view! {
                        <button class="ac-footer-btn" on:click=move |_| cb.run(())>
                            "Open Needs-You Queue"
                        </button>
                    })}
                </div>
            </div>
        </ModalOverlay>
    }
}

fn trigger_label_short(trigger: &str) -> &'static str {
    match trigger {
        "scheduled" | "cron" => "Sched",
        "manual" => "Manual",
        _ => "Other",
    }
}
