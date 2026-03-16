//! SessionDashboard — grid/card-based dashboard of all sessions.
//! Matches React `SessionDashboard.tsx`.

use crate::api::client::{api_fetch, api_post_void};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{SessionOverviewEntry, SessionsOverviewResponse};
use leptos::prelude::*;
use std::collections::HashSet;
use crate::components::icons::*;

fn relative_time(ts: f64) -> String {
    let now = js_sys::Date::now() / 1000.0;
    let diff = now - ts;
    if diff < 60.0 {
        "just now".to_string()
    } else if diff < 3600.0 {
        format!("{}m ago", (diff / 60.0).floor() as i64)
    } else if diff < 86400.0 {
        format!("{}h ago", (diff / 3600.0).floor() as i64)
    } else {
        format!("{}d ago", (diff / 86400.0).floor() as i64)
    }
}

fn format_cost(cost: f64) -> String {
    format!("${:.2}", cost)
}

/// SessionDashboard component.
#[component]
pub fn SessionDashboard(
    on_select_session: Callback<(usize, String)>,
    on_close: Callback<()>,
    active_session_id: Option<String>,
) -> impl IntoView {
    let (data, set_data) = signal(None::<SessionsOverviewResponse>);
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);
    let (sort_by, set_sort_by) = signal("activity".to_string());
    let (aborting, set_aborting) = signal(HashSet::<String>::new());

    let load = move || {
        leptos::task::spawn_local(async move {
            match api_fetch::<SessionsOverviewResponse>("/sessions/overview").await {
                Ok(resp) => {
                    set_data.set(Some(resp));
                    set_error.set(None);
                }
                Err(e) => {
                    set_error.set(Some(format!("Failed to load sessions: {}", e)));
                }
            }
            set_loading.set(false);
        });
    };

    // Initial load
    let load_init = load.clone();
    load_init();

    // Auto-refresh every 5s using a spawned loop (wasm-compatible, no Send+Sync issues)
    let load_interval = load.clone();
    leptos::task::spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            load_interval();
        }
    });

    let load_refresh = load.clone();
    let load_abort = load.clone();

    let active_id = active_session_id.clone();

    let handle_abort = move |session_id: String| {
        let sid = session_id.clone();
        set_aborting.update(|s| {
            s.insert(sid.clone());
        });
        let load = load_abort.clone();
        leptos::task::spawn_local(async move {
            let _ = api_post_void(
                &format!("/session/{}/abort", sid),
                &serde_json::json!({}),
            )
            .await;
            load();
            set_aborting.update(|s| {
                s.remove(&sid);
            });
        });
    };

    view! {
        <ModalOverlay on_close=on_close class="session-dashboard-panel">
            // Header
            <div class="session-dashboard-header">
                <h2>"Session Dashboard"</h2>
                <div class="session-dashboard-stats">
                    <span>
                        <IconActivity size=12 class="w-3 h-3" />
                        {move || data.get().map(|d| format!("{} sessions", d.total)).unwrap_or("- sessions".into())}
                    </span>
                    <span>
                        <svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="12" cy="12" r="10"/></svg>
                        {move || data.get().map(|d| format!("{} busy", d.busy_count)).unwrap_or("- busy".into())}
                    </span>
                </div>
                <button class="session-dashboard-refresh" on:click=move |_| { set_loading.set(true); load_refresh(); }>
                    <svg class=move || format!("w-3.5 h-3.5{}", if loading.get() { " spinning" } else { "" }) viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
                    </svg>
                </button>
                <button class="session-dashboard-close" on:click=move |_| on_close.run(())>
                    <IconX size=16 class="w-4 h-4" />
                </button>
            </div>

            // Sort controls
            <div class="session-dashboard-controls">
                <label for="sd-sort">"Sort by"</label>
                <select id="sd-sort" prop:value=move || sort_by.get()
                    on:change=move |e| set_sort_by.set(event_target_value(&e))>
                    <option value="activity">"Recent Activity"</option>
                    <option value="cost">"Cost"</option>
                    <option value="project">"Project"</option>
                </select>
            </div>

            // Error
            {move || error.get().map(|err| view! { <div class="session-dashboard-error">{err}</div> })}

            // Body
            {move || {
                if loading.get() && data.get().is_none() {
                    return view! {
                        <div class="session-dashboard-loading">
                            <IconLoader2 size=20 class="w-5 h-5 spinning" />
                            "Loading sessions..."
                        </div>
                    }.into_any();
                }

                let d = data.get();
                let sort = sort_by.get();
                let active = active_id.clone();
                let aborting_snap = aborting.get();

                let mut sessions = d.as_ref().map(|d| d.sessions.clone()).unwrap_or_default();
                match sort.as_str() {
                    "cost" => sessions.sort_by(|a, b| {
                        let ca = a.stats.as_ref().map(|s| s.cost).unwrap_or(0.0);
                        let cb = b.stats.as_ref().map(|s| s.cost).unwrap_or(0.0);
                        cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
                    }),
                    "project" => sessions.sort_by(|a, b| {
                        a.project_name.cmp(&b.project_name).then_with(|| {
                            b.time.updated.partial_cmp(&a.time.updated).unwrap_or(std::cmp::Ordering::Equal)
                        })
                    }),
                    _ => sessions.sort_by(|a, b| {
                        b.time.updated.partial_cmp(&a.time.updated).unwrap_or(std::cmp::Ordering::Equal)
                    }),
                }

                if sessions.is_empty() {
                    return view! { <div class="session-dashboard-empty">"No sessions found."</div> }.into_any();
                }

                view! {
                    <div class="session-dashboard-grid">
                        {sessions.iter().map(|session| {
                            let is_active = active.as_deref() == Some(&session.id);
                            let is_aborting = aborting_snap.contains(&session.id);
                            let cls = format!("session-dashboard-card{}{}",
                                if is_active { " active" } else { "" },
                                if session.is_busy { " busy" } else { "" });
                            let title = if session.title.is_empty() { session.id.chars().take(8).collect::<String>() } else { session.title.clone() };
                            let cost_str = format_cost(session.stats.as_ref().map(|s| s.cost).unwrap_or(0.0));
                            let time_str = relative_time(session.time.updated);
                            let sid = session.id.clone();
                            let sid2 = session.id.clone();
                            let pidx = session.project_index;
                            let is_busy = session.is_busy;
                            let project_name = session.project_name.clone();
                            view! {
                                <button class=cls on:click=move |_| on_select_session.run((pidx, sid.clone()))>
                                    <div class="session-dashboard-card-title">
                                        <span class=format!("session-dashboard-status-dot {}", if is_busy { "busy" } else { "idle" })/>
                                        <span class="session-dashboard-card-text">{title}</span>
                                    </div>
                                    <span class="session-dashboard-card-project">{project_name}</span>
                                    <div class="session-dashboard-card-meta">
                                        <span class="session-dashboard-card-cost">
                                            <IconDollarSign size=12 class="w-3 h-3" />
                                            {cost_str}
                                        </span>
                                        <span class="session-dashboard-card-time">
                                            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
                                            </svg>
                                            {time_str}
                                        </span>
                                    </div>
                                    {if is_busy {
                                        let ha = handle_abort.clone();
                                        Some(view! {
                                            <button class="session-dashboard-card-stop"
                                                on:click=move |e: web_sys::MouseEvent| { e.stop_propagation(); ha(sid2.clone()); }
                                                disabled=is_aborting>
                                                {if is_aborting {
                                                    view! {
                                                         <IconLoader2 size=12 class="w-3 h-3 spinning" />
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <circle cx="12" cy="12" r="10"/><rect x="9" y="9" width="6" height="6" rx="1"/>
                                                        </svg>
                                                    }.into_any()
                                                }}
                                                "Stop"
                                            </button>
                                        })
                                    } else { None }}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}

            // Footer
            <div class="session-dashboard-footer">
                <IconDollarSign size=12 class="w-3 h-3" />
                {move || {
                    let total_cost = data.get()
                        .map(|d| d.sessions.iter().map(|s| s.stats.as_ref().map(|st| st.cost).unwrap_or(0.0)).sum::<f64>())
                        .unwrap_or(0.0);
                    format!("Total cost: {}", format_cost(total_cost))
                }}
            </div>
        </ModalOverlay>
    }
}
