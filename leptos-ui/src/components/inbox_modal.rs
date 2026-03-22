//! InboxModal — prioritized inbox of pending items requiring attention.
//! Matches React `inbox-modal/InboxModal.tsx`.
//! Simplified Leptos port: fetches inbox items from POST /inbox, allows filtering.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_post, api_post_void};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{InboxItem, InboxResponse};
use crate::utils::format_relative_time as format_time;

// ── Helpers ─────────────────────────────────────────────────────────

fn priority_color(priority: &str) -> &'static str {
    match priority {
        "high" | "critical" => "var(--color-error, #e05252)",
        "medium" => "var(--color-warning, #e6a817)",
        _ => "var(--color-text-muted, #999)",
    }
}

fn source_icon(source: &str) -> &'static str {
    match source {
        "permission" => "\u{1F6E1}",
        "question" => "\u{2753}",
        "signal" | "assistant" => "\u{1F4E1}",
        "mission" => "\u{1F3AF}",
        "watcher" => "\u{1F441}",
        _ => "\u{1F4E5}",
    }
}

// ── API body ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ComputeInboxBody {
    permissions: Vec<String>,
    questions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    watcher_status: Option<String>,
    signals: Vec<String>,
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn InboxModal(
    on_close: Callback<()>,
    #[prop(optional)]
    on_open_missions: Option<Callback<()>>,
) -> impl IntoView {
    let (items, set_items) = signal(Vec::<InboxItem>::new());
    let (loading, set_loading) = signal(true);
    let (filter, set_filter) = signal("all".to_string());

    // Load on mount
    {
        leptos::task::spawn_local(async move {
            let body = ComputeInboxBody {
                permissions: vec![],
                questions: vec![],
                watcher_status: None,
                signals: vec![],
            };
            match api_post::<InboxResponse>("/inbox", &body).await {
                Ok(resp) => set_items.set(resp.items),
                Err(e) => leptos::logging::warn!("Failed to load inbox: {}", e),
            }
            set_loading.set(false);
        });
    }

    let filtered_items = Memo::new(move |_| {
        let all = items.get();
        let f = filter.get();
        match f.as_str() {
            "needs-you" => all.into_iter().filter(|i| i.state == "unresolved" || i.state == "pending").collect::<Vec<_>>(),
            "high" => all.into_iter().filter(|i| i.priority == "high" || i.priority == "critical").collect::<Vec<_>>(),
            _ => all,
        }
    });

    view! {
        <ModalOverlay on_close=on_close class="assistant-inbox-modal">
            <div class="assistant-inbox-header">
                <div class="assistant-inbox-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="22 12 16 12 14 15 10 15 8 12 2 12" />
                        <path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" />
                    </svg>
                    <h3>"Assistant Inbox"</h3>
                    <span class="assistant-inbox-count">{move || filtered_items.get().len()}</span>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close inbox">
                    <IconX size=16 />
                </button>
            </div>

            // Filter bar
            <div class="assistant-inbox-filters">
                <button
                    class=move || if filter.get() == "all" { "assistant-inbox-filter active" } else { "assistant-inbox-filter" }
                    on:click=move |_| set_filter.set("all".to_string())
                >"All"</button>
                <button
                    class=move || if filter.get() == "needs-you" { "assistant-inbox-filter active" } else { "assistant-inbox-filter" }
                    on:click=move |_| set_filter.set("needs-you".to_string())
                >"Needs You"</button>
                <button
                    class=move || if filter.get() == "high" { "assistant-inbox-filter active" } else { "assistant-inbox-filter" }
                    on:click=move |_| set_filter.set("high".to_string())
                >"High Priority"</button>
            </div>

            <div class="assistant-inbox-scrollable">
                <div class="assistant-inbox-body">
                    {move || {
                        if loading.get() {
                            return view! { <div class="assistant-inbox-empty">"Loading inbox..."</div> }.into_any();
                        }
                        let current = filtered_items.get();
                        if current.is_empty() {
                            return view! {
                                <div class="assistant-inbox-empty">"Nothing needs attention right now."</div>
                            }.into_any();
                        }

                        let rows = current.iter().map(|item| {
                            let icon = source_icon(&item.source);
                            let color = priority_color(&item.priority);
                            let title = item.title.clone();
                            let desc = item.description.clone();
                            let ts = format_time(item.created_at);
                            let state = item.state.clone();
                            let prio = item.priority.clone();

                            view! {
                                <div class="assistant-inbox-item">
                                    <div class="assistant-inbox-item-icon" style=format!("color: {}", color)>
                                        {icon}
                                    </div>
                                    <div class="assistant-inbox-item-main">
                                        <div class="assistant-inbox-item-row">
                                            <span class="assistant-inbox-item-title">{title}</span>
                                            <span class="assistant-inbox-priority" style=format!("color: {}", color)>
                                                {prio}
                                            </span>
                                        </div>
                                        <div class="assistant-inbox-item-desc">{desc}</div>
                                        <div class="assistant-inbox-item-meta">
                                            <span class="assistant-inbox-item-state">{state}</span>
                                            <span class="assistant-inbox-item-time">{ts}</span>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>();

                        view! { <div>{rows}</div> }.into_any()
                    }}
                </div>
            </div>

            // Footer
            <div class="assistant-inbox-footer">
                {on_open_missions.map(|cb| view! {
                    <button class="assistant-inbox-footer-btn" on:click=move |_| cb.run(())>
                        "View Missions"
                    </button>
                })}
            </div>
        </ModalOverlay>
    }
}
