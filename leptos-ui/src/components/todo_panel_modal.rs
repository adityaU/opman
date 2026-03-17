//! TodoPanelModal — full CRUD todo management per session.
//! Matches React `TodoPanelModal.tsx`.

use crate::api::client::{api_fetch, api_post};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use leptos::prelude::*;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
struct TodoItem {
    #[serde(default)]
    id: String,
    #[serde(rename = "sessionID", default)]
    session_id: String,
    content: String,
    status: String,
    #[serde(default)]
    priority: Option<String>,
}

fn next_status(current: &str) -> &'static str {
    match current {
        "pending" => "in_progress",
        "in_progress" => "completed",
        "completed" => "pending",
        "cancelled" => "pending",
        _ => "pending",
    }
}

fn next_priority(current: Option<&str>) -> &'static str {
    match current {
        Some("low") => "medium",
        Some("medium") => "high",
        Some("high") => "low",
        _ => "low",
    }
}

fn status_icon_class(status: &str) -> &'static str {
    match status {
        "completed" => "todo-icon todo-icon-completed",
        "in_progress" => "todo-icon todo-icon-progress",
        "cancelled" => "todo-icon todo-icon-cancelled",
        _ => "todo-icon todo-icon-pending",
    }
}

fn priority_class(priority: Option<&str>) -> &'static str {
    match priority {
        Some("high") => "todo-priority todo-priority-high",
        Some("medium") => "todo-priority todo-priority-medium",
        Some("low") => "todo-priority todo-priority-low",
        _ => "todo-priority todo-priority-none",
    }
}

/// TodoPanelModal component.
#[component]
pub fn TodoPanelModal(on_close: Callback<()>, session_id: String) -> impl IntoView {
    let (todos, set_todos) = signal(Vec::<TodoItem>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);
    let (selected_idx, set_selected_idx) = signal(0usize);
    // None = not editing, Some(None) = creating, Some(Some(i)) = editing i
    let (editing_idx, set_editing_idx) = signal(None::<Option<usize>>);
    let (editing_content, set_editing_content) = signal(String::new());
    let (editing_priority, set_editing_priority) = signal("medium".to_string());
    let (copy_flash, set_copy_flash) = signal(false);

    let sid = session_id.clone();
    let sid2 = session_id.clone();

    // Load todos
    Effect::new(move |_| {
        let sid = sid.clone();
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match api_fetch::<Vec<TodoItem>>(&format!("/session/{}/todos", sid)).await {
                Ok(items) => {
                    set_todos.set(items);
                    set_loading.set(false);
                }
                Err(_) => {
                    set_error.set(Some("Failed to load todos".into()));
                    set_loading.set(false);
                }
            }
        });
    });

    // Persist helper — stored so it can be reused from multiple closures.
    let sid_persist = StoredValue::new(sid2.clone());
    let persist_todos_fn = move |items: &[TodoItem]| {
        let sid = sid_persist.get_value();
        let payload: Vec<serde_json::Value> = items
            .iter()
            .map(|t| {
                serde_json::json!({
                    "content": t.content,
                    "status": t.status,
                    "priority": t.priority.as_deref().unwrap_or("medium"),
                })
            })
            .collect();
        leptos::task::spawn_local(async move {
            let _ = api_post::<serde_json::Value>(
                &format!("/session/{}/todos", sid),
                &payload,
            )
            .await;
        });
    };

    // Mutate helper — uses only Copy captures so it can be copied.
    let mutate = move |updater: Box<dyn FnOnce(Vec<TodoItem>) -> Vec<TodoItem>>| {
        set_todos.update(|prev| {
            let next = updater(std::mem::take(prev));
            persist_todos_fn(&next);
            *prev = next;
        });
    };

    let toggle_status = move |idx: usize| {
        mutate(Box::new(move |mut items| {
            if let Some(t) = items.get_mut(idx) {
                t.status = next_status(&t.status).to_string();
            }
            items
        }));
    };

    let cycle_priority = move |idx: usize| {
        mutate(Box::new(move |mut items| {
            if let Some(t) = items.get_mut(idx) {
                t.priority = Some(next_priority(t.priority.as_deref()).to_string());
            }
            items
        }));
    };

    let delete_todo = move |idx: usize| {
        mutate(Box::new(move |mut items| {
            if idx < items.len() {
                items.remove(idx);
            }
            items
        }));
        set_selected_idx.update(|sel| {
            let new_len = todos.get_untracked().len();
            if new_len == 0 {
                *sel = 0;
            } else if *sel >= new_len {
                *sel = new_len - 1;
            }
        });
    };

    let move_up = move |idx: usize| {
        if idx == 0 {
            return;
        }
        mutate(Box::new(move |mut items| {
            if idx > 0 && idx < items.len() {
                items.swap(idx - 1, idx);
            }
            items
        }));
        set_selected_idx.set(idx - 1);
    };

    let move_down = move |idx: usize| {
        mutate(Box::new(move |mut items| {
            if idx < items.len().saturating_sub(1) {
                items.swap(idx, idx + 1);
            }
            items
        }));
        set_selected_idx.update(|sel| *sel += 1);
    };

    let start_create = move || {
        set_editing_idx.set(Some(None));
        set_editing_content.set(String::new());
        set_editing_priority.set("medium".to_string());
    };

    let start_edit = move |idx: usize| {
        let items = todos.get_untracked();
        if let Some(t) = items.get(idx) {
            set_editing_idx.set(Some(Some(idx)));
            set_editing_content.set(t.content.clone());
            set_editing_priority.set(t.priority.clone().unwrap_or("medium".into()));
        }
    };

    let sid_commit = StoredValue::new(sid2.clone());
    let commit_edit = move || {
        let editing = editing_idx.get_untracked();
        if editing.is_none() {
            return;
        }
        let content = editing_content.get_untracked().trim().to_string();
        if content.is_empty() {
            set_editing_idx.set(None);
            return;
        }
        let edit_target = editing.unwrap();
        let priority = editing_priority.get_untracked();
        match edit_target {
            None => {
                // Create new
                let new_todo = TodoItem {
                    id: format!("temp-{}", js_sys::Date::now() as u64),
                    session_id: sid_commit.get_value(),
                    content,
                    status: "pending".to_string(),
                    priority: Some(priority),
                };
                let len = todos.get_untracked().len();
                mutate(Box::new(move |mut items| {
                    items.push(new_todo);
                    items
                }));
                set_selected_idx.set(len);
            }
            Some(idx) => {
                mutate(Box::new(move |mut items| {
                    if let Some(t) = items.get_mut(idx) {
                        t.content = content;
                        t.priority = Some(priority);
                    }
                    items
                }));
            }
        }
        set_editing_idx.set(None);
    };

    let short_id = if session_id.chars().count() > 12 {
        session_id.chars().take(12).collect::<String>()
    } else {
        session_id.clone()
    };

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let is_editing = editing_idx.get().is_some();
        if is_editing {
            if e.key() == "Enter" {
                e.prevent_default();
                commit_edit();
            }
            return;
        }

        let key = e.key();
        match key.as_str() {
            "ArrowDown" | "j" => {
                e.prevent_default();
                let len = todos.get_untracked().len();
                if len > 0 {
                    set_selected_idx.update(|i| *i = (*i + 1).min(len - 1));
                }
            }
            "ArrowUp" | "k" => {
                e.prevent_default();
                set_selected_idx.update(|i| *i = i.saturating_sub(1));
            }
            " " => {
                e.prevent_default();
                let len = todos.get_untracked().len();
                if len > 0 {
                    toggle_status(selected_idx.get_untracked());
                }
            }
            "n" => {
                e.prevent_default();
                start_create();
            }
            "e" => {
                e.prevent_default();
                let len = todos.get_untracked().len();
                if len > 0 {
                    start_edit(selected_idx.get_untracked());
                }
            }
            "d" => {
                e.prevent_default();
                let len = todos.get_untracked().len();
                if len > 0 {
                    delete_todo(selected_idx.get_untracked());
                }
            }
            "p" => {
                e.prevent_default();
                let len = todos.get_untracked().len();
                if len > 0 {
                    cycle_priority(selected_idx.get_untracked());
                }
            }
            "K" => {
                e.prevent_default();
                move_up(selected_idx.get_untracked());
            }
            "J" => {
                e.prevent_default();
                move_down(selected_idx.get_untracked());
            }
            _ => {}
        }
    };

    view! {
        <ModalOverlay on_close=on_close class="todo-panel-modal">
            <div on:keydown=on_keydown tabindex="0" style="outline: none;">
                // Header
                <div class="todo-panel-header">
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="9 11 12 14 22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/>
                    </svg>
                    <span>"Todos"</span>
                    <span class="todo-panel-session">{short_id}</span>
                    {move || if copy_flash.get() { Some(view! { <span class="todo-copy-flash">"Copied!"</span> }) } else { None }}
                    <button class="todo-panel-action-btn" on:click=move |_| start_create() title="New todo (n)">
                        <IconPlus size=14 class="w-3.5 h-3.5" />
                    </button>
                    <button class="todo-panel-close" on:click=move |_| on_close.run(())>
                        <IconX size=14 class="w-3.5 h-3.5" />
                    </button>
                </div>

                // Summary
                {move || {
                    let items = todos.get();
                    if !loading.get() && error.get().is_none() && !items.is_empty() {
                        let total = items.len();
                        let completed = items.iter().filter(|t| t.status == "completed").count();
                        let in_progress = items.iter().filter(|t| t.status == "in_progress").count();
                        let pending = items.iter().filter(|t| t.status == "pending").count();
                        let cancelled = items.iter().filter(|t| t.status == "cancelled").count();
                        Some(view! {
                            <div class="todo-panel-summary">
                                <span class="todo-summary-item">{format!("{} total", total)}</span>
                                <span class="todo-summary-item todo-summary-completed">{format!("{} done", completed)}</span>
                                <span class="todo-summary-item todo-summary-progress">{format!("{} active", in_progress)}</span>
                                <span class="todo-summary-item todo-summary-pending">{format!("{} pending", pending)}</span>
                                {if cancelled > 0 {
                                    Some(view! { <span class="todo-summary-item todo-summary-cancelled">{format!("{} cancelled", cancelled)}</span> })
                                } else { None }}
                            </div>
                        })
                    } else {
                        None
                    }
                }}

                // List
                <div class="todo-panel-list">
                    {move || {
                        if loading.get() {
                            return view! {
                                <div class="todo-panel-empty">
                                    <IconLoader2 size=16 class="w-4 h-4 spinning" />
                                    <span>"Loading todos..."</span>
                                </div>
                            }.into_any();
                        }
                        if let Some(err) = error.get() {
                            return view! { <div class="todo-panel-empty todo-panel-error">{err}</div> }.into_any();
                        }
                        let items = todos.get();
                        let selected = selected_idx.get();
                        let editing = editing_idx.get();
                        if items.is_empty() && editing.is_none() {
                            return view! {
                                <div class="todo-panel-empty">
                                    "No todos for this session"
                                    <button class="todo-panel-create-btn" on:click=move |_| start_create()>
                                        <IconPlus size=12 class="w-3 h-3" />
                                        " Create one"
                                    </button>
                                </div>
                            }.into_any();
                        }

                        view! {
                            <div>
                                {items.iter().enumerate().map(|(idx, todo)| {
                                    let is_selected = idx == selected;
                                    let is_editing = matches!(editing, Some(Some(i)) if i == idx);
                                    let cls = format!("todo-panel-item{} todo-status-{}", if is_selected { " selected" } else { "" }, todo.status);
                                    let status = todo.status.clone();
                                    let priority = todo.priority.clone();
                                    let content = todo.content.clone();
                                    view! {
                                        <div class=cls on:click=move |_| set_selected_idx.set(idx) on:dblclick=move |_| start_edit(idx)>
                                            // Status icon
                                            <span class=status_icon_class(&status) on:click=move |e: web_sys::MouseEvent| { e.stop_propagation(); toggle_status(idx); }>
                                                {status_svg(&status)}
                                            </span>
                                            // Priority icon
                                            <span class=priority_class(priority.as_deref()) on:click=move |e: web_sys::MouseEvent| { e.stop_propagation(); cycle_priority(idx); }>
                                                {priority_svg(priority.as_deref())}
                                            </span>
                                            // Content or edit input
                                            {if is_editing {
                                                view! {
                                                    <input class="todo-edit-input"
                                                        prop:value=move || editing_content.get()
                                                        on:input=move |e| set_editing_content.set(event_target_value(&e))
                                                        on:keydown=move |e: web_sys::KeyboardEvent| {
                                                            if e.key() == "Enter" { e.prevent_default(); e.stop_propagation(); commit_edit(); }
                                                            else if e.key() == "Escape" { e.prevent_default(); e.stop_propagation(); set_editing_idx.set(None); }
                                                        }
                                                        placeholder="Todo content..."
                                                    />
                                                }.into_any()
                                            } else {
                                                view! { <span class="todo-panel-content">{content}</span> }.into_any()
                                            }}
                                        </div>
                                    }
                                }).collect_view()}

                                // Inline create row
                                {if matches!(editing, Some(None)) {
                                    Some(view! {
                                        <div class="todo-panel-item todo-panel-item-editing selected">
                                            <span class="todo-icon todo-icon-pending">
                                                <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>
                                            </span>
                                            <span class=priority_class(Some(&editing_priority.get_untracked()))>
                                                {priority_svg(Some(&editing_priority.get_untracked()))}
                                            </span>
                                            <input class="todo-edit-input"
                                                prop:value=move || editing_content.get()
                                                on:input=move |e| set_editing_content.set(event_target_value(&e))
                                                on:keydown=move |e: web_sys::KeyboardEvent| {
                                                    if e.key() == "Enter" { e.prevent_default(); e.stop_propagation(); commit_edit(); }
                                                    else if e.key() == "Escape" { e.prevent_default(); e.stop_propagation(); set_editing_idx.set(None); }
                                                }
                                                placeholder="New todo..."
                                            />
                                        </div>
                                    })
                                } else { None }}
                            </div>
                        }.into_any()
                    }}
                </div>

                // Footer
                <div class="todo-panel-footer">
                    <kbd>"Space"</kbd>" Toggle "
                    <kbd>"n"</kbd>" New "
                    <kbd>"e"</kbd>" Edit "
                    <kbd>"d"</kbd>" Delete "
                    <kbd>"p"</kbd>" Priority "
                    <kbd>"K/J"</kbd>" Reorder "
                    <kbd>"Esc"</kbd>" Close"
                </div>
            </div>
        </ModalOverlay>
    }
}

fn status_svg(status: &str) -> impl IntoView {
    match status {
        "completed" => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="9 11 12 14 22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/>
            </svg>
        }
        .into_any(),
        "in_progress" => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
            </svg>
        }
        .into_any(),
        "cancelled" => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
            </svg>
        }
        .into_any(),
        _ => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>
        }
        .into_any(),
    }
}

fn priority_svg(priority: Option<&str>) -> impl IntoView {
    match priority {
        Some("high") => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/>
            </svg>
        }
        .into_any(),
        Some("medium") => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/>
            </svg>
        }
        .into_any(),
        Some("low") => view! {
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 19 5 12"/>
            </svg>
        }
        .into_any(),
        _ => view! { <span>"·"</span> }.into_any(),
    }
}
