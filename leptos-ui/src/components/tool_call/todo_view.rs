//! TodoWrite view — collapsible accordion with "n/m completed" header.
//! The todo list items render in the body, same style as before.

use super::sub_components::TodoList;
use crate::components::icons::*;
use crate::components::message_timeline::AccordionState;
use crate::hooks::use_auto_open::{AutoOpenState, ToolCategory};
use crate::types::core::MessagePart;
use leptos::prelude::*;

/// Parse todo items from input JSON, return (completed, total).
fn count_todos(input: &serde_json::Value) -> (usize, usize) {
    let items = if let Some(obj) = input.as_object() {
        obj.get("todos").cloned().unwrap_or(input.clone())
    } else {
        input.clone()
    };
    let arr = match items.as_array() {
        Some(a) => a,
        None => return (0, 0),
    };
    let total = arr.len();
    let completed = arr
        .iter()
        .filter(|item| {
            item.get("status")
                .and_then(|s| s.as_str())
                .map_or(false, |s| s == "completed")
        })
        .count();
    (completed, total)
}

/// Render a todowrite tool call as a collapsible accordion.
pub fn render_todo_tool(part: &MessagePart) -> leptos::prelude::AnyView {
    let input_data = part.state.as_ref().and_then(|s| s.input.clone());
    let has_input = input_data.as_ref().map_or(false, |d| match d {
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Object(m) => !m.is_empty(),
        _ => false,
    });

    if !has_input {
        return view! {}.into_any();
    }

    let inp = input_data.unwrap_or(serde_json::Value::Null);
    let (completed, total) = count_todos(&inp);
    let all_done = total > 0 && completed == total;

    // Auto-open / accordion state
    let status = part
        .state
        .as_ref()
        .and_then(|s| s.status.as_deref())
        .unwrap_or("pending");
    let is_completed = status == "completed";
    let is_running = status == "running" || status == "pending";

    let auto_open_config = use_context::<AutoOpenState>()
        .map(|s| s.get())
        .unwrap_or_default();

    let initial_expanded = crate::components::tool_call::helpers::auto_expand_default(
        Some(ToolCategory::TodoWrite),
        is_running,
        is_completed,
        false,
        false,
        &auto_open_config,
    );

    let accordion_key = part
        .tool_call_id
        .clone()
        .or_else(|| part.call_id.clone())
        .or_else(|| part.id.clone())
        .unwrap_or_default();
    let accordion_ctx = use_context::<AccordionState>();
    let default_expanded = if let Some(AccordionState(map)) = accordion_ctx {
        match map.with_untracked(|m| m.get(&accordion_key).copied()) {
            Some(val) => val,
            None => {
                map.update_untracked(|m| {
                    m.insert(accordion_key.clone(), initial_expanded);
                });
                initial_expanded
            }
        }
    } else {
        initial_expanded
    };
    let (expanded, set_expanded) = signal(default_expanded);
    let ak = accordion_key.clone();
    let handle_toggle = move |_: web_sys::MouseEvent| {
        let new_val = !expanded.get_untracked();
        set_expanded.set(new_val);
        if let Some(AccordionState(map)) = accordion_ctx {
            map.update(|m| {
                m.insert(ak.clone(), new_val);
            });
        }
    };

    let header_text = format!("{}/{} completed", completed, total);
    let status_class = if all_done {
        "todo-acc-status done"
    } else {
        "todo-acc-status"
    };

    view! {
        <div class="todo-view">
            <div class="todo-list">
                {move || expanded.get().then(|| {
                    let inp_clone = inp.clone();
                    view! { <TodoList input=inp_clone /> }
                })}
                <button class="todo-acc-header" on:click=handle_toggle>
                    <span class="todo-acc-chevron">
                        {move || if expanded.get() {
                            view! { <IconChevronDown size=12 /> }.into_any()
                        } else {
                            view! { <IconChevronRight size=12 /> }.into_any()
                        }}
                    </span>
                    <span class=status_class>
                        {if all_done {
                            view! { <IconCheckCircle2 size=11 class="todo-acc-check" /> }.into_any()
                        } else {
                            view! { <IconCircleDot size=11 /> }.into_any()
                        }}
                        {header_text}
                    </span>
                </button>
            </div>
        </div>
    }
    .into_any()
}
