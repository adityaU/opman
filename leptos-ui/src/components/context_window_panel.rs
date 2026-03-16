//! ContextWindowPanel — token usage gauge and category breakdown.
//! Matches React `context-window-panel/ContextWindowPanel.tsx`.

use crate::api::client::{api_fetch, api_post_void};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{ContextCategory, ContextWindowResponse};
use leptos::prelude::*;
use std::f64::consts::PI;
use crate::components::icons::*;

const GAUGE_RADIUS: f64 = 54.0;
const GAUGE_CIRCUMFERENCE: f64 = 2.0 * PI * GAUGE_RADIUS;

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn category_color(color: &str) -> &'static str {
    match color {
        "blue" => "var(--color-info)",
        "green" => "var(--color-success)",
        "orange" => "var(--color-warning)",
        "purple" => "var(--color-accent)",
        "gray" => "var(--color-text-muted)",
        _ => "var(--color-text-muted)",
    }
}

#[component]
pub fn ContextWindowPanel(
    on_close: Callback<()>,
    session_id: Option<String>,
    on_compact: Callback<()>,
) -> impl IntoView {
    let (data, set_data) = signal(None::<ContextWindowResponse>);
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);
    let (compacting, set_compacting) = signal(false);

    let sid = session_id.clone();
    let load_data = move || {
        let sid = sid.clone();
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            let path = match &sid {
                Some(id) => format!("/session/{}/context-window", id),
                None => "/context-window".to_string(),
            };
            match api_fetch::<ContextWindowResponse>(&path).await {
                Ok(resp) => {
                    set_data.set(Some(resp));
                    set_loading.set(false);
                }
                Err(_) => {
                    set_error.set(Some("Failed to load context window data".into()));
                    set_loading.set(false);
                }
            }
        });
    };
    let load_data_init = load_data.clone();
    load_data_init();

    let load_data_refresh = load_data.clone();
    let load_data_compact = load_data.clone();

    let sid_compact = session_id.clone();
    let handle_compact = Callback::new(move |_: ()| {
        let sid = sid_compact.clone();
        if sid.is_none() || compacting.get_untracked() {
            return;
        }
        set_compacting.set(true);
        let load = load_data_compact.clone();
        let on_compact = on_compact;
        leptos::task::spawn_local(async move {
            if let Some(id) = &sid {
                let _ = api_post_void(
                    &format!("/session/{}/command/compact", id),
                    &serde_json::json!({}),
                )
                .await;
                on_compact.run(());
                // Delay reload
                gloo_timers::future::TimeoutFuture::new(1000).await;
                load();
            }
            set_compacting.set(false);
        });
    });

    let has_session = session_id.is_some();

    view! {
        <ModalOverlay on_close=on_close class="ctx-window-modal">
            // Header
            <div class="ctx-window-header">
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
                </svg>
                <span>"Context Window"</span>
                <div class="ctx-window-header-actions">
                    <button class="ctx-window-refresh" on:click=move |_| load_data_refresh() title="Refresh">
                        <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
                        </svg>
                    </button>
                    <button class="ctx-window-close" on:click=move |_| on_close.run(())>
                        <IconX size=14 class="w-3.5 h-3.5" />
                    </button>
                </div>
            </div>

            {move || {
                if loading.get() {
                    view! {
                        <div class="ctx-window-loading">
                            <IconLoader2 size=16 class="w-4 h-4 spinning" />
                            <span>"Loading context data..."</span>
                        </div>
                    }.into_any()
                } else if let Some(err) = error.get() {
                    view! {
                        <div class="ctx-window-error">{err}</div>
                    }.into_any()
                } else if let Some(d) = data.get() {
                    let gauge_color = if d.usage_pct > 90.0 {
                        "var(--color-error)"
                    } else if d.usage_pct > 70.0 {
                        "var(--color-warning)"
                    } else {
                        "var(--color-success)"
                    };
                    let usage_fraction = (d.usage_pct / 100.0).min(1.0);
                    let stroke_dashoffset = GAUGE_CIRCUMFERENCE * (1.0 - usage_fraction);
                    let pct_text = format!("{}%", d.usage_pct.round() as i64);
                    let used_text = format_tokens(d.total_used);
                    let limit_text = format_tokens(d.context_limit);
                    let remaining = if d.context_limit > d.total_used { d.context_limit - d.total_used } else { 0 };
                    let remaining_text = format_tokens(remaining);
                    let est = d.estimated_messages_remaining;
                    let categories = d.categories.clone();
                    let categories2 = d.categories.clone();
                    let ctx_limit = d.context_limit;
                    let total_used = d.total_used;

                    view! {
                        <div>
                            // Gauge + summary
                            <div class="ctx-window-gauge-section">
                                <div class="ctx-window-gauge">
                                    <svg viewBox="0 0 128 128" class="ctx-gauge-svg">
                                        <circle cx="64" cy="64" r=GAUGE_RADIUS.to_string() fill="none" stroke="var(--color-border)" stroke-width="8"/>
                                        <circle cx="64" cy="64" r=GAUGE_RADIUS.to_string() fill="none"
                                            stroke=gauge_color stroke-width="8"
                                            stroke-dasharray=GAUGE_CIRCUMFERENCE.to_string()
                                            stroke-dashoffset=stroke_dashoffset.to_string()
                                            stroke-linecap="round"
                                            transform="rotate(-90 64 64)"
                                            class="ctx-gauge-ring"/>
                                        <text x="64" y="58" text-anchor="middle" class="ctx-gauge-pct" fill=gauge_color>
                                            {pct_text}
                                        </text>
                                        <text x="64" y="76" text-anchor="middle" class="ctx-gauge-label" fill="var(--color-text-muted)">
                                            "used"
                                        </text>
                                    </svg>
                                </div>
                                <div class="ctx-window-summary">
                                    <div class="ctx-breakdown-title">"Session Budget"</div>
                                    <div class="ctx-summary-row">
                                        <span class="ctx-summary-label">"Used"</span>
                                        <span class="ctx-summary-value">{used_text}</span>
                                    </div>
                                    <div class="ctx-summary-row">
                                        <span class="ctx-summary-label">"Limit"</span>
                                        <span class="ctx-summary-value">{limit_text}</span>
                                    </div>
                                    <div class="ctx-summary-row">
                                        <span class="ctx-summary-label">"Remaining"</span>
                                        <span class="ctx-summary-value">{remaining_text}</span>
                                    </div>
                                    {est.map(|n| view! {
                                        <div class="ctx-summary-row ctx-summary-estimate">
                                            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
                                            </svg>
                                            <span>{format!("~{} messages remaining", n)}</span>
                                        </div>
                                    })}
                                </div>
                            </div>

                            // Category breakdown
                            <div class="ctx-window-breakdown">
                                <div class="ctx-breakdown-title">"Token Breakdown"</div>
                                <div class="ctx-stacked-bar">
                                    {categories.iter().map(|cat| {
                                        let w = if ctx_limit > 0 { (cat.tokens as f64 / ctx_limit as f64) * 100.0 } else { 0.0 };
                                        let w_str = format!("width: {}%; background-color: {};", w.max(0.5), category_color(&cat.color));
                                        let title = format!("{}: {} ({}%)", cat.label, format_tokens(cat.tokens), format!("{:.1}", cat.pct));
                                        view! {
                                            <div class="ctx-stacked-segment" style=w_str title=title/>
                                        }
                                    }).collect_view()}
                                    {if total_used < ctx_limit {
                                        let rem_w = ((ctx_limit - total_used) as f64 / ctx_limit as f64) * 100.0;
                                        Some(view! {
                                            <div class="ctx-stacked-segment ctx-stacked-remaining" style=format!("width: {}%;", rem_w)/>
                                        })
                                    } else { None }}
                                </div>
                                {categories2.iter().map(|cat| {
                                    view! { <CategoryRow category=cat.clone() context_limit=ctx_limit /> }
                                }).collect_view()}
                            </div>

                            // Actions
                            {if has_session {
                                Some(view! {
                                    <div class="ctx-window-actions">
                                        <button class="ctx-action-btn" on:click=move |_| handle_compact.run(()) disabled=move || compacting.get()>
                                            {move || if compacting.get() {
                                                view! {
                                                     <IconLoader2 size=12 class="w-3 h-3 spinning" />
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                        <polyline points="4 14 10 14 10 20"/><polyline points="20 10 14 10 14 4"/>
                                                        <line x1="14" y1="10" x2="21" y2="3"/><line x1="3" y1="21" x2="10" y2="14"/>
                                                    </svg>
                                                }.into_any()
                                            }}
                                            <span>"Compact History"</span>
                                        </button>
                                    </div>
                                })
                            } else { None }}
                        </div>
                    }.into_any()
                } else {
                    view! { <div class="ctx-window-error">"No data available"</div> }.into_any()
                }
            }}

            // Footer
            <div class="ctx-window-footer">
                <kbd>"Esc"</kbd>" Close "
                <kbd>"R"</kbd>" Refresh"
            </div>
        </ModalOverlay>
    }
}

/// Individual category row with expandable items.
#[component]
fn CategoryRow(category: ContextCategory, context_limit: u64) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    let bar_width = if context_limit > 0 {
        (category.tokens as f64 / context_limit as f64) * 100.0
    } else {
        0.0
    };
    let has_items = !category.items.is_empty();
    let items = category.items.clone();
    let color = category_color(&category.color).to_string();
    let color2 = color.clone();
    let _color3 = color.clone();
    let label = category.label.clone();
    let tokens_str = format_tokens(category.tokens);
    let pct_str = format!("{:.1}%", category.pct);

    view! {
        <div class="ctx-category">
            <button class="ctx-category-header" on:click=move |_| if has_items { set_expanded.update(|v| *v = !*v) }>
                <span class="ctx-category-expand">
                    {move || if !has_items {
                        view! { <span style="width: 12px; display: inline-block;"></span> }.into_any()
                    } else if expanded.get() {
                        view! {
                            <IconChevronDown size=12 class="w-3 h-3" />
                        }.into_any()
                    } else {
                        view! {
                            <IconChevronRight size=12 class="w-3 h-3" />
                        }.into_any()
                    }}
                </span>
                <span class="ctx-category-dot" style=format!("background-color: {};", color)/>
                <span class="ctx-category-label">{label}</span>
                <span class="ctx-category-tokens">{tokens_str}</span>
                <span class="ctx-category-pct">{pct_str}</span>
            </button>
            <div class="ctx-category-bar-track">
                <div class="ctx-category-bar-fill" style=format!("width: {}%; background-color: {};", bar_width.min(100.0), color2)/>
            </div>
            {move || {
                if expanded.get() && !items.is_empty() {
                    let items_clone = items.clone();
                    Some(view! {
                        <div class="ctx-category-items">
                            {items_clone.iter().map(|item| {
                                let lbl = item.label.clone();
                                let tok = format_tokens(item.tokens);
                                view! {
                                    <div class="ctx-item">
                                        <span class="ctx-item-label">{lbl}</span>
                                        <span class="ctx-item-tokens">{tok}</span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    })
                } else {
                    None
                }
            }}
        </div>
    }
}
