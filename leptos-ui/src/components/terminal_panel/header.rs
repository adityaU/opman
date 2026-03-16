//! Terminal panel header views — tab bar, header actions, search bar.
//! Matches React terminal-panel/components.tsx.

use leptos::prelude::*;

use crate::components::icons::*;

use super::types::{kind_label, TabInfo, TabStatus, ALL_PTY_KINDS};

/// Render the tab bar with tabs, rename input, close buttons, and new-tab menu.
pub fn render_tab_bar(
    tabs: ReadSignal<Vec<TabInfo>>,
    active_tab_id: ReadSignal<Option<String>>,
    set_active_tab_id: WriteSignal<Option<String>>,
    rename_id: ReadSignal<Option<String>>,
    rename_value: ReadSignal<String>,
    set_rename_value: WriteSignal<String>,
    start_rename: Callback<String>,
    commit_rename: Callback<()>,
    cancel_rename: Callback<()>,
    close_tab: Callback<String>,
    kind_menu_open: ReadSignal<bool>,
    set_kind_menu_open: WriteSignal<bool>,
    create_tab: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="term-tab-bar">
            {move || {
                let current_tabs = tabs.get();
                let active = active_tab_id.get();
                let ren_id = rename_id.get();
                let tab_count = current_tabs.len();

                current_tabs.iter().map(|tab| {
                    let tid_click = tab.id.clone();
                    let tid_dbl = tab.id.clone();
                    let tid_close = tab.id.clone();
                    let label = tab.label.clone();
                    let label_title = tab.label.clone();
                    let kind = tab.kind.clone();
                    let is_active = active.as_deref() == Some(&tab.id);
                    let is_renaming = ren_id.as_deref() == Some(&tab.id);

                    view! {
                        <div
                            class=if is_active { "term-tab active" } else { "term-tab" }
                            on:click=move |_| set_active_tab_id.set(Some(tid_click.clone()))
                            on:dblclick=move |_| start_rename.run(tid_dbl.clone())
                            title=format!("{} \u{2014} {}", kind_label(&kind), label_title)
                        >
                            <IconTerminal size=11 class="term-tab-icon" />
                            {if is_renaming {
                                view! {
                                    <input
                                        class="term-tab-rename"
                                        prop:value=move || rename_value.get()
                                        on:input=move |e| set_rename_value.set(event_target_value(&e))
                                        on:blur=move |_| commit_rename.run(())
                                        on:keydown=move |e: web_sys::KeyboardEvent| {
                                            if e.key() == "Enter" { commit_rename.run(()); }
                                            if e.key() == "Escape" { cancel_rename.run(()); }
                                        }
                                        on:click=move |e: web_sys::MouseEvent| e.stop_propagation()
                                        autofocus=true
                                    />
                                }.into_any()
                            } else {
                                view! { <span class="term-tab-label">{label}</span> }.into_any()
                            }}
                            {(tab_count > 1).then(|| {
                                view! {
                                    <button class="term-tab-close"
                                        on:click=move |e: web_sys::MouseEvent| {
                                            e.stop_propagation();
                                            close_tab.run(tid_close.clone());
                                        }
                                        title="Close tab"
                                    ><IconX size=10 /></button>
                                }
                            })}
                        </div>
                    }
                }).collect::<Vec<_>>()
            }}
            // New tab + kind menu
            <div class="term-tab-new-wrapper">
                <button class="term-tab-new"
                    on:click=move |_| set_kind_menu_open.update(|v| *v = !*v)
                    title="New terminal tab"
                ><IconPlus size=12 /></button>
                {move || kind_menu_open.get().then(|| {
                    view! {
                        <div class="term-kind-menu">
                            {ALL_PTY_KINDS.iter().map(|k| {
                                let kind_c = k.to_string();
                                let lbl = kind_label(k);
                                view! {
                                    <button class="term-kind-item"
                                        on:click=move |_| create_tab.run(kind_c.clone())>
                                        {lbl}
                                    </button>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }
                })}
            </div>
        </div>
    }
}

/// Render header action buttons (MCP indicator, search toggle, expand, close).
pub fn render_header_actions(
    mcp_agent_active: Option<Signal<bool>>,
    search_open: ReadSignal<bool>,
    toggle_search: Callback<()>,
    expanded: ReadSignal<bool>,
    set_expanded: WriteSignal<bool>,
    close_all: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="terminal-panel-actions">
            {move || {
                let active = mcp_agent_active.map(|s| s.get()).unwrap_or(false);
                active.then(|| view! {
                    <span class="mcp-agent-indicator" title="AI agent active">
                        <span class="mcp-agent-dot" />
                    </span>
                })
            }}
            <button
                class=move || if search_open.get() { "active" } else { "" }
                on:click=move |_| toggle_search.run(())
                title="Find (Cmd+F)"
                aria-label="Search in terminal"
            ><IconSearch size=14 /></button>
            <button
                on:click=move |_| set_expanded.update(|v| *v = !*v)
                title="Toggle size"
                aria-label=move || if expanded.get() { "Minimize terminal" } else { "Maximize terminal" }
            >
                {move || if expanded.get() {
                    view! {
                        <svg width="14" height="14" viewBox="0 0 24 24"
                            fill="none" stroke="currentColor" stroke-width="2"
                            stroke-linecap="round" stroke-linejoin="round">
                            <polyline points="4 14 10 14 10 20" />
                            <polyline points="20 10 14 10 14 4" />
                            <line x1="14" y1="10" x2="21" y2="3" />
                            <line x1="3" y1="21" x2="10" y2="14" />
                        </svg>
                    }.into_any()
                } else {
                    view! {
                        <svg width="14" height="14" viewBox="0 0 24 24"
                            fill="none" stroke="currentColor" stroke-width="2"
                            stroke-linecap="round" stroke-linejoin="round">
                            <polyline points="15 3 21 3 21 9" />
                            <polyline points="9 21 3 21 3 15" />
                            <line x1="21" y1="3" x2="14" y2="10" />
                            <line x1="3" y1="21" x2="10" y2="14" />
                        </svg>
                    }.into_any()
                }}
            </button>
            <button
                on:click=move |_| close_all.run(())
                title="Close terminal panel"
                aria-label="Close terminal panel"
            ><IconX size=14 /></button>
        </div>
    }
}

/// Render the search bar (when open).
pub fn render_search_bar(
    search_query: ReadSignal<String>,
    search_change: Callback<String>,
    search_next: Callback<()>,
    search_prev: Callback<()>,
    close_search: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="term-search-bar">
            <IconSearch size=12 class="term-search-icon" />
            <input
                class="term-search-input"
                type="text"
                placeholder="Find in terminal..."
                prop:value=move || search_query.get()
                on:input=move |e| search_change.run(event_target_value(&e))
                on:keydown=move |e: web_sys::KeyboardEvent| {
                    if e.key() == "Enter" {
                        e.prevent_default();
                        if e.shift_key() { search_prev.run(()); } else { search_next.run(()); }
                    }
                    if e.key() == "Escape" { e.prevent_default(); close_search.run(()); }
                }
            />
            <button class="term-search-nav"
                on:click=move |_| search_prev.run(())
                title="Previous match (Shift+Enter)">
                <svg width="14" height="14" viewBox="0 0 24 24"
                    fill="none" stroke="currentColor" stroke-width="2"
                    stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="18 15 12 9 6 15" />
                </svg>
            </button>
            <button class="term-search-nav"
                on:click=move |_| search_next.run(())
                title="Next match (Enter)">
                <IconChevronDown size=14 />
            </button>
            <button class="term-search-close"
                on:click=move |_| close_search.run(())
                title="Close search">
                <IconX size=12 />
            </button>
        </div>
    }
}
