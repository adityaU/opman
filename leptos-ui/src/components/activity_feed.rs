//! ActivityFeed — real-time activity event stream for a session.
//! Matches React `ActivityFeed.tsx`.

use crate::api::client::api_fetch;
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::ActivityEvent;
use leptos::prelude::*;

#[derive(Clone)]
struct KindConfig {
    color: &'static str,
    label: &'static str,
    icon_path: &'static str,
}

fn kind_config(kind: &str) -> KindConfig {
    match kind {
        "file_edit" => KindConfig {
            color: "var(--color-info)",
            label: "File Edit",
            icon_path: "M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z",
        },
        "tool_call" => KindConfig {
            color: "var(--color-warning)",
            label: "Tool Call",
            icon_path: "M13 2 3 14h9l-1 8 10-12h-9l1-8z",
        },
        "terminal" => KindConfig {
            color: "var(--color-success)",
            label: "Terminal",
            icon_path: "m4 17 6-6-6-6M12 19h8",
        },
        "permission" => KindConfig {
            color: "var(--color-accent)",
            label: "Permission",
            icon_path: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
        },
        "question" => KindConfig {
            color: "var(--color-text-muted)",
            label: "Question",
            icon_path: "M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3M12 17h.01",
        },
        _ => KindConfig {
            color: "var(--color-text-secondary)",
            label: "Status",
            icon_path: "M22 12h-4l-3 9L9 3l-3 9H2",
        },
    }
}

fn relative_time(iso: &str) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    let diff = (js_sys::Date::now() - d.get_time()) / 1000.0;
    if diff < 60.0 {
        format!("{}s ago", diff.round() as i64)
    } else if diff < 3600.0 {
        format!("{}m ago", (diff / 60.0).round() as i64)
    } else {
        format!("{}h ago", (diff / 3600.0).round() as i64)
    }
}

fn format_time(iso: &str) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    let h = d.get_hours();
    let m = d.get_minutes();
    let s = d.get_seconds();
    format!("{:02}:{:02}:{:02}", h, m, s)
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ActivityFeedResponse {
    events: Vec<ActivityEvent>,
}

/// ActivityFeed component.
#[component]
pub fn ActivityFeed(
    session_id: Option<String>,
    on_close: Callback<()>,
) -> impl IntoView {
    let (events, set_events) = signal(Vec::<ActivityEvent>::new());
    let (loading, set_loading) = signal(true);
    let (auto_scroll, set_auto_scroll) = signal(true);

    // Fetch initial events
    let sid = session_id.clone();
    Effect::new(move |_| {
        let sid = sid.clone();
        if let Some(id) = sid {
            leptos::task::spawn_local(async move {
                match api_fetch::<ActivityFeedResponse>(&format!("/session/{}/activity", id))
                    .await
                {
                    Ok(resp) => set_events.set(resp.events),
                    Err(_) => {}
                }
                set_loading.set(false);
            });
        } else {
            set_loading.set(false);
        }
    });

    let all_kinds: Vec<&'static str> = vec![
        "file_edit",
        "tool_call",
        "terminal",
        "permission",
        "question",
        "status",
    ];

    view! {
        <ModalOverlay on_close=on_close class="activity-feed-panel">
            <div class="activity-feed-header">
                <h3>"Activity Feed"</h3>
                <div class="activity-feed-header-right">
                    {move || if !auto_scroll.get() {
                        Some(view! {
                            <button class="activity-feed-scroll-btn" on:click=move |_| set_auto_scroll.set(true)>
                                "Scroll to bottom"
                            </button>
                        })
                    } else { None }}
                    <button on:click=move |_| on_close.run(())>
                        <IconX size=14 class="w-3.5 h-3.5" />
                    </button>
                </div>
            </div>

            <div class="activity-feed-body"
                on:scroll=move |_| {
                    // Simple auto-scroll tracking would go here in full impl
                }>
                {move || {
                    if loading.get() {
                        return view! { <div class="activity-feed-empty">"Loading activity..."</div> }.into_any();
                    }
                    let evts = events.get();
                    if evts.is_empty() {
                        return view! { <div class="activity-feed-empty">"No activity yet for this session."</div> }.into_any();
                    }

                    view! {
                        <div>
                            {evts.iter().map(|ev| {
                                let cfg = kind_config(&ev.kind);
                                let time_rel = relative_time(&ev.timestamp);
                                let time_full = format_time(&ev.timestamp);
                                let summary = ev.summary.clone();
                                let detail = ev.detail.clone();
                                let color = cfg.color;
                                let icon_path = cfg.icon_path;
                                let label = cfg.label;
                                view! {
                                    <div class="activity-feed-event">
                                        <span class="activity-feed-event-icon" style=format!("color: {};", color) title=label>
                                            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                <path d=icon_path/>
                                            </svg>
                                        </span>
                                        <span class="activity-feed-event-time" title=time_full>{time_rel}</span>
                                        <span class="activity-feed-event-summary">{summary}</span>
                                        {detail.map(|d| { let d2 = d.clone(); view! {
                                            <span class="activity-feed-event-detail" title=d>{d2}</span>
                                        }})}
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>

            <div class="activity-feed-footer">
                <span>{move || {
                    let count = events.get().len();
                    format!("{} event{}", count, if count != 1 { "s" } else { "" })
                }}</span>
                <span class="activity-feed-legend">
                    {all_kinds.iter().map(|k| {
                        let cfg = kind_config(k);
                        view! {
                            <span class="activity-feed-legend-item" style=format!("color: {};", cfg.color)>
                                <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d=cfg.icon_path/>
                                </svg>
                                " " {cfg.label}
                            </span>
                        }
                    }).collect_view()}
                </span>
            </div>
        </ModalOverlay>
    }
}
