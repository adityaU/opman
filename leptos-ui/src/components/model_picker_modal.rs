//! ModelPickerModal — browse & select AI models from connected providers.
//! Matches React `ModelPickerModal.tsx`.

use leptos::prelude::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;

// ── Wire types (match actual /providers API) ────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct ApiModelInfo {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    context_length: u64,
    #[serde(default)]
    max_output_tokens: Option<u64>,
    // Also tolerate the `limit` shape from core::Provider
    #[serde(default)]
    limit: Option<ApiModelLimit>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiModelLimit {
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiProvider {
    id: String,
    name: String,
    #[serde(default)]
    models: HashMap<String, ApiModelInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProvidersApiResponse {
    #[serde(default)]
    all: Vec<ApiProvider>,
    #[serde(default)]
    connected: Vec<String>,
    #[serde(default, rename = "default")]
    defaults: HashMap<String, String>,
}

// ── View-layer flat model ───────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct FlatModel {
    provider_id: String,
    provider_name: String,
    model_id: String,
    model_name: String,
    context_window: Option<u64>,
    output_limit: Option<u64>,
    is_connected: bool,
    is_default: bool,
}

// ── Component ───────────────────────────────────────────────────────

/// Model picker modal component.
#[component]
pub fn ModelPickerModal(
    on_close: Callback<()>,
    session_id: Option<String>,
    on_model_selected: Callback<(String, String)>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (selected_index, set_selected_index) = signal(0usize);
    let (show_all, set_show_all) = signal(false);
    let (providers, set_providers) = signal::<Vec<ApiProvider>>(Vec::new());
    let (connected, set_connected) = signal::<HashSet<String>>(HashSet::new());
    let (defaults, set_defaults) = signal::<HashMap<String, String>>(HashMap::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal::<Option<String>>(None);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus input
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Fetch providers on mount — strongly-typed deserialization
    {
        leptos::task::spawn_local(async move {
            match crate::api::api_fetch::<ProvidersApiResponse>("/providers").await {
                Ok(resp) => {
                    set_providers.set(resp.all);
                    set_connected.set(resp.connected.into_iter().collect());
                    set_defaults.set(resp.defaults);
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(e.message));
                    set_loading.set(false);
                }
            }
        });
    }

    // Flatten models memo
    let flat_models = Memo::new(move |_| {
        let provs = providers.get();
        let conn = connected.get();
        let defs = defaults.get();
        let show = show_all.get();

        let mut flat: Vec<FlatModel> = Vec::new();
        for p in &provs {
            let is_connected = conn.contains(&p.id);
            if !show && !is_connected {
                continue;
            }
            for (mid, minfo) in &p.models {
                let is_default = defs.get(&p.id).map_or(false, |d| d == mid);
                let display_name = minfo.name.clone().unwrap_or_else(|| mid.clone());
                // Support both API shapes for context/output limits
                let ctx = if minfo.context_length > 0 {
                    Some(minfo.context_length)
                } else {
                    minfo.limit.as_ref().and_then(|l| l.context)
                };
                let out = minfo.max_output_tokens
                    .or_else(|| minfo.limit.as_ref().and_then(|l| l.output));
                flat.push(FlatModel {
                    provider_id: p.id.clone(),
                    provider_name: p.name.clone(),
                    model_id: mid.clone(),
                    model_name: display_name,
                    context_window: ctx,
                    output_limit: out,
                    is_connected,
                    is_default,
                });
            }
        }

        // Sort: defaults first, then alphabetical
        flat.sort_by(|a, b| {
            b.is_default.cmp(&a.is_default)
                .then(a.model_name.cmp(&b.model_name))
        });
        flat
    });

    let filtered = Memo::new(move |_| {
        let models = flat_models.get();
        let q = query.get().to_lowercase();
        if q.is_empty() {
            return models;
        }
        models
            .into_iter()
            .filter(|m| {
                m.model_id.to_lowercase().contains(&q)
                    || m.model_name.to_lowercase().contains(&q)
                    || m.provider_name.to_lowercase().contains(&q)
                    || m.provider_id.to_lowercase().contains(&q)
            })
            .collect()
    });

    let connected_count = Memo::new(move |_| {
        let provs = providers.get();
        let conn = connected.get();
        provs.iter()
            .filter(|p| conn.contains(&p.id))
            .map(|p| p.models.len())
            .sum::<usize>()
    });

    let all_count = Memo::new(move |_| {
        providers.get().iter().map(|p| p.models.len()).sum::<usize>()
    });

    // Reset on query/tab change
    Effect::new(move |_| {
        let _ = query.get();
        let _ = show_all.get();
        set_selected_index.set(0);
    });

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 {
                    set_selected_index.update(|i| *i = (*i + 1).min(len - 1));
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                set_selected_index.update(|i| *i = i.saturating_sub(1));
            }
            "Enter" => {
                e.prevent_default();
                let items = filtered.get_untracked();
                let idx = selected_index.get_untracked();
                if session_id.is_some() {
                    if let Some(model) = items.get(idx) {
                        on_model_selected.run((model.model_id.clone(), model.provider_id.clone()));
                        on_close.run(());
                    }
                }
            }
            "Tab" => {
                e.prevent_default();
                set_show_all.update(|v| *v = !*v);
            }
            _ => {}
        }
    };

    let format_k = |n: u64| -> String {
        if n >= 1000 { format!("{}K", n / 1000) } else { format!("{}", n) }
    };

    view! {
        <ModalOverlay on_close=on_close class="model-picker">
            <div class="model-picker-header">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect width="16" height="16" x="4" y="4" rx="2"/><rect width="6" height="6" x="9" y="9" rx="1"/>
                    <path d="M15 2v2"/><path d="M15 20v2"/><path d="M2 15h2"/><path d="M2 9h2"/><path d="M20 15h2"/><path d="M20 9h2"/><path d="M9 2v2"/><path d="M9 20v2"/>
                </svg>
                <span>"Choose Model"</span>
                <span class="model-picker-count">
                    {move || format!("{} model(s)", filtered.get().len())}
                </span>
            </div>
            <div class="model-picker-input-row">
                <IconSearch size=14 class="w-3.5 h-3.5" />
                <input
                    class="model-picker-input"
                    node_ref=input_ref
                    type="text"
                    placeholder="Search models..."
                    prop:value=move || query.get()
                    on:input=move |e| set_query.set(event_target_value(&e))
                    on:keydown=on_keydown
                />
            </div>
            <div class="model-picker-tabs">
                <button
                    class=move || if !show_all.get() { "model-picker-tab active" } else { "model-picker-tab" }
                    on:click=move |_| set_show_all.set(false)
                >
                    {move || format!("Connected ({})", connected_count.get())}
                </button>
                <button
                    class=move || if show_all.get() { "model-picker-tab active" } else { "model-picker-tab" }
                    on:click=move |_| set_show_all.set(true)
                >
                    {move || format!("All Providers ({})", all_count.get())}
                </button>
                <span class="model-picker-tab-hint">"Tab to switch"</span>
            </div>
            <div class="model-picker-results">
                {move || {
                    if loading.get() {
                        view! { <div class="model-picker-empty">"Loading providers..."</div> }.into_any()
                    } else if let Some(err) = error.get() {
                        view! { <div class="model-picker-empty model-picker-error">{err}</div> }.into_any()
                    } else {
                        let items = filtered.get();
                        let sel = selected_index.get();
                        if items.is_empty() {
                            view! { <div class="model-picker-empty">"No models found"</div> }.into_any()
                        } else {
                            items.into_iter().enumerate().map(|(idx, model)| {
                                let is_selected = idx == sel;
                                let mid = model.model_id.clone();
                                let pid = model.provider_id.clone();
                                let class_str = if is_selected { "model-picker-item selected" } else { "model-picker-item" };
                                let ctx_str = model.context_window.map(|n| format_k(n) + " ctx");
                                let out_str = model.output_limit.map(|n| format_k(n) + " out");
                                let show_id = model.model_id != model.model_name;
                                let model_id_display = model.model_id.clone();

                                view! {
                                    <button
                                        class=class_str
                                        on:click=move |_| {
                                            on_model_selected.run((mid.clone(), pid.clone()));
                                            on_close.run(());
                                        }
                                        on:mouseenter=move |_| set_selected_index.set(idx)
                                    >
                                        <div class="model-picker-item-left">
                                            <span class="model-picker-name">
                                                {if model.is_default {
                                                    Some(view! {
                                                         <IconCheck size=10 class="model-default-icon w-2.5 h-2.5 inline mr-1" />
                                                    })
                                                } else {
                                                    None
                                                }}
                                                {model.model_name.clone()}
                                            </span>
                                            <span class="model-picker-provider">
                                                {model.provider_name.clone()}
                                                {if show_id {
                                                    Some(view! { <span class="model-picker-id">{format!(" · {}", model_id_display)}</span> })
                                                } else {
                                                    None
                                                }}
                                            </span>
                                        </div>
                                        <div class="model-picker-item-right">
                                            {ctx_str.map(|s| view! { <span class="model-picker-ctx">{s}</span> })}
                                            {out_str.map(|s| view! { <span class="model-picker-out">{s}</span> })}
                                        </div>
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }
                }}
            </div>
        </ModalOverlay>
    }
}
