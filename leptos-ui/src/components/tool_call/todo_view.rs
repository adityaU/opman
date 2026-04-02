//! Dedicated todowrite view — renders directly without accordion wrapper.
//! Shows the todo list inline with no surrounding chrome.

use super::sub_components::TodoList;
use crate::types::core::MessagePart;
use leptos::prelude::*;

/// Render a todowrite tool call directly — no accordion, no header.
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

    view! {
        <div class="todo-view">
            <TodoList input=inp />
        </div>
    }
    .into_any()
}
