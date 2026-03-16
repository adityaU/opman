//! SessionGraph — collapsible tree of all sessions across projects.
//! Matches React `SessionGraph.tsx`.

use crate::api::client::api_fetch;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{SessionTreeNode, SessionsTreeResponse};
use leptos::prelude::*;
use std::collections::HashSet;
use crate::components::icons::*;

fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// SessionGraph component.
#[component]
pub fn SessionGraph(
    on_select_session: Callback<(usize, String)>,
    on_close: Callback<()>,
    active_session_id: Option<String>,
) -> impl IntoView {
    let (tree, set_tree) = signal(None::<SessionsTreeResponse>);
    let (loading, set_loading) = signal(true);
    let (collapsed, set_collapsed) = signal(HashSet::<String>::new());

    // Fetch tree on mount
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api_fetch::<SessionsTreeResponse>("/sessions/tree").await {
                Ok(data) => set_tree.set(Some(data)),
                Err(_) => {}
            }
            set_loading.set(false);
        });
    });

    let toggle_collapse = Callback::new(move |id: String| {
        set_collapsed.update(|s| {
            if s.contains(&id) {
                s.remove(&id);
            } else {
                s.insert(id);
            }
        });
    });

    view! {
        <ModalOverlay on_close=on_close class="session-graph-panel">
            // Header
            <div class="session-graph-header">
                <IconGitBranch size=14 class="w-3.5 h-3.5" />
                <span>"Session Graph"</span>
                {move || tree.get().map(|t| view! {
                    <span class="session-graph-count">
                        {format!("{} session{}", t.total, if t.total != 1 { "s" } else { "" })}
                    </span>
                })}
                <button class="session-graph-close" on:click=move |_| on_close.run(())>
                        <IconX size=14 class="w-3.5 h-3.5" />
                </button>
            </div>

            // Body
            <div class="session-graph-body">
                {move || {
                    if loading.get() {
                        return view! {
                            <div class="session-graph-loading">
                                <IconLoader2 size=16 class="w-4 h-4 spinning" />
                                <span>"Loading session tree..."</span>
                            </div>
                        }.into_any();
                    }

                    match tree.get() {
                        Some(t) if !t.roots.is_empty() => {
                            let active_id = active_session_id.clone();
                            let collapsed_snap = collapsed.get();
                            view! {
                                <div>
                                    {t.roots.into_iter().map(|root| {
                                        render_tree_node(root, 0, active_id.clone(), collapsed_snap.clone(), on_select_session, on_close, toggle_collapse)
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                        _ => view! { <div class="session-graph-empty">"No sessions found"</div> }.into_any(),
                    }
                }}
            </div>

            // Footer
            <div class="session-graph-footer">
                <kbd>"Esc"</kbd>" Close"
                <span class="session-graph-legend">
                    <svg class="w-2 h-2 session-graph-legend-busy" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="12" cy="12" r="12"/></svg>
                    " busy "
                    <svg class="w-2 h-2 session-graph-legend-idle" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="12" cy="12" r="12"/></svg>
                    " idle"
                </span>
            </div>
        </ModalOverlay>
    }
}

fn render_tree_node(
    node: SessionTreeNode,
    depth: usize,
    active_session_id: Option<String>,
    collapsed: HashSet<String>,
    on_select_session: Callback<(usize, String)>,
    on_close: Callback<()>,
    toggle_collapse: Callback<String>,
) -> impl IntoView {
    let is_active = active_session_id.as_deref() == Some(node.id.as_str());
    let is_collapsed = collapsed.contains(&node.id);
    let has_children = !node.children.is_empty();
    let title = if node.title.is_empty() {
        node.id.chars().take(8).collect::<String>()
    } else {
        node.title.clone()
    };
    let project_name = node.project_name.clone();
    let cost = node.stats.as_ref().map(|s| s.cost).unwrap_or(0.0);
    let is_busy = node.is_busy;
    let node_id = node.id.clone();
    let node_id_toggle = node.id.clone();
    let project_index = node.project_index;

    let padding = format!("padding-left: {}px;", if depth > 0 { depth * 16 } else { 0 });
    let cls = format!(
        "session-graph-node{}",
        if is_active { " active" } else { "" }
    );

    // Recursively render children (only if expanded)
    let children_view = if has_children && !is_collapsed {
        let child_views: Vec<_> = node.children.into_iter().map(|child| {
            render_tree_node(child, depth + 1, active_session_id.clone(), collapsed.clone(), on_select_session, on_close, toggle_collapse)
        }).collect();
        Some(view! {
            <div class="session-graph-children">
                {child_views.into_iter().map(|v| v.into_any()).collect_view()}
            </div>
        })
    } else {
        None
    };

    view! {
        <div class="session-graph-tree-branch" style=padding>
            <button class=cls on:click=move |_| {
                on_select_session.run((project_index, node_id.clone()));
                on_close.run(());
            }>
                <span class="session-graph-toggle" on:click=move |e: leptos::ev::MouseEvent| {
                    if has_children {
                        e.stop_propagation();
                        toggle_collapse.run(node_id_toggle.clone());
                    }
                }>
                    {if has_children {
                        if is_collapsed {
                            view! { <IconChevronRight size=12 class="w-3 h-3" /> }.into_any()
                        } else {
                            view! { <IconChevronDown size=12 class="w-3 h-3" /> }.into_any()
                        }
                    } else {
                        view! { <span style="width: 13px; display: inline-block;"></span> }.into_any()
                    }}
                </span>
                <div class="session-graph-node-info">
                    <span class="session-graph-node-title">{title}</span>
                    <span class="session-graph-project-tag">{project_name}</span>
                    {if cost > 0.0 {
                        Some(view! { <span class="session-graph-cost">{format_cost(cost)}</span> })
                    } else { None }}
                </div>
                <span class=format!("session-graph-status {}", if is_busy { "busy" } else { "idle" })>
                    <svg class="w-2 h-2" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="12" cy="12" r="12"/></svg>
                </span>
            </button>

            {children_view}
        </div>
    }
}
