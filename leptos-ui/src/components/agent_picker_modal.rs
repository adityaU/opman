//! AgentPickerModal — browse & select agent modes.
//! Matches React `AgentPickerModal.tsx`.

use leptos::prelude::*;
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::AgentInfo;

// ── Component ───────────────────────────────────────────────────────

/// Agent picker modal component.
#[component]
pub fn AgentPickerModal(
    on_close: Callback<()>,
    current_agent: String,
    on_agent_selected: Callback<String>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (selected_index, set_selected_index) = signal(0usize);
    let (all_agents, set_all_agents) = signal::<Vec<AgentInfo>>(Vec::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal::<Option<String>>(None);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus input
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Fetch agents on mount
    {
        leptos::task::spawn_local(async move {
            match crate::api::api_fetch::<Vec<AgentInfo>>("/agents").await {
                Ok(agents) => {
                    set_all_agents.set(agents);
                    set_loading.set(false);
                }
                Err(e) => {
                    // Fallback to defaults
                    set_all_agents.set(vec![
                        AgentInfo { id: "build".to_string(), label: "Build".to_string(), description: "General-purpose coding agent".to_string(), mode: Some("primary".to_string()), hidden: None, native: Some(true), color: None },
                        AgentInfo { id: "plan".to_string(), label: "Plan".to_string(), description: "Planning and research agent".to_string(), mode: Some("primary".to_string()), hidden: None, native: Some(true), color: None },
                    ]);
                    set_error.set(Some(e.message));
                    set_loading.set(false);
                }
            }
        });
    }

    // Filter: exclude subagents and hidden
    let current = current_agent.clone();
    let agents = Memo::new(move |_| {
        let all = all_agents.get();
        let mut selectable: Vec<AgentInfo> = all
            .into_iter()
            .filter(|a| {
                a.mode.as_deref() != Some("subagent") && !a.hidden.unwrap_or(false)
            })
            .collect();

        // Sort: current first, then alphabetical
        let cur = current.clone();
        selectable.sort_by(|a, b| {
            let a_is_cur = a.id == cur;
            let b_is_cur = b.id == cur;
            b_is_cur.cmp(&a_is_cur).then(a.label.cmp(&b.label))
        });
        selectable
    });

    let current2 = current_agent.clone();
    let filtered = Memo::new(move |_| {
        let list = agents.get();
        let q = query.get().to_lowercase();
        if q.is_empty() {
            return list;
        }
        list.into_iter()
            .filter(|a| {
                a.id.to_lowercase().contains(&q)
                    || a.label.to_lowercase().contains(&q)
                    || a.description.to_lowercase().contains(&q)
            })
            .collect()
    });

    // Reset on query change
    Effect::new(move |_| {
        let _ = query.get();
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
                if let Some(agent) = items.get(idx) {
                    on_agent_selected.run(agent.id.clone());
                    on_close.run(());
                }
            }
            _ => {}
        }
    };

    let current3 = current_agent;

    view! {
        <ModalOverlay on_close=on_close class="agent-picker">
            <div class="agent-picker-header">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M12 8V4H8"/><rect width="16" height="12" x="4" y="8" rx="2"/>
                    <path d="M2 14h2"/><path d="M20 14h2"/><path d="M15 13v2"/><path d="M9 13v2"/>
                </svg>
                <span>"Choose Agent"</span>
                <span class="agent-picker-count">
                    {move || format!("{} agent(s)", filtered.get().len())}
                </span>
            </div>
            <div class="agent-picker-input-row">
                <IconSearch size=14 class="w-3.5 h-3.5" />
                <input
                    class="agent-picker-input"
                    node_ref=input_ref
                    type="text"
                    placeholder="Search agents..."
                    prop:value=move || query.get()
                    on:input=move |e| set_query.set(event_target_value(&e))
                    on:keydown=on_keydown
                />
            </div>
            <div class="agent-picker-results">
                {move || {
                    if loading.get() {
                        view! { <div class="agent-picker-empty">"Loading agents..."</div> }.into_any()
                    } else if let Some(err) = error.get() {
                        view! { <div class="agent-picker-empty agent-picker-error">{format!("Error: {}", err)}</div> }.into_any()
                    } else {
                        let items = filtered.get();
                        let sel = selected_index.get();
                        let current_id = current3.clone();
                        if items.is_empty() {
                            view! { <div class="agent-picker-empty">"No agents found"</div> }.into_any()
                        } else {
                            items.into_iter().enumerate().map(|(idx, agent)| {
                                let is_selected = idx == sel;
                                let is_current = agent.id == current_id;
                                let class_str = if is_selected { "agent-picker-item selected" } else { "agent-picker-item" };
                                let aid = agent.id.clone();
                                let show_id = agent.id.to_lowercase() != agent.label.to_lowercase();
                                let id_display = agent.id.clone();
                                let has_mode = agent.mode.as_ref().map_or(false, |m| m != "primary");
                                let mode_display = agent.mode.clone().unwrap_or_default();
                                let is_native = agent.native.unwrap_or(false);
                                let dot_color = agent.color.clone();

                                view! {
                                    <button
                                        class=class_str
                                        on:click=move |_| {
                                            on_agent_selected.run(aid.clone());
                                            on_close.run(());
                                        }
                                        on:mouseenter=move |_| set_selected_index.set(idx)
                                    >
                                        <div class="agent-picker-item-left">
                                            <span class="agent-picker-name">
                                                {if is_current {
                                                    Some(view! {
                                                        <IconCheck size=10 class="agent-current-icon w-2.5 h-2.5 inline mr-1" />
                                                    })
                                                } else {
                                                    None
                                                }}
                                                {if let Some(ref color) = dot_color {
                                                    Some(view! { <span class="agent-picker-dot" style=format!("background-color: {}", color)></span> })
                                                } else {
                                                    None
                                                }}
                                                {agent.label.clone()}
                                            </span>
                                            <span class="agent-picker-desc">
                                                {agent.description.clone()}
                                                {if show_id {
                                                    Some(view! { <span class="agent-picker-id">{format!(" · {}", id_display)}</span> })
                                                } else {
                                                    None
                                                }}
                                            </span>
                                        </div>
                                        <div class="agent-picker-item-right">
                                            {if has_mode {
                                                Some(view! { <span class="agent-picker-mode">{mode_display.clone()}</span> })
                                            } else {
                                                None
                                            }}
                                            {if is_native {
                                                Some(view! { <span class="agent-picker-native">"built-in"</span> })
                                            } else {
                                                None
                                            }}
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
