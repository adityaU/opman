//! A2UI — agent-to-UI block renderer for `ui_render` tool calls.
//!
//! Renders structured UI blocks (cards, tables, key-value pairs, status
//! indicators, progress bars, alerts, markdown, buttons, forms) inline
//! in the session timeline.

mod blocks;
mod interactive;

use leptos::prelude::*;

/// Top-level A2UI renderer. Parses the `blocks` array from tool input.
#[component]
pub fn A2uiBlocks(input: serde_json::Value) -> impl IntoView {
    let blocks = extract_blocks(&input);
    if blocks.is_empty() {
        return view! {
            <div class="a2ui-empty">"No UI blocks"</div>
        }
        .into_any();
    }

    view! {
        <div class="a2ui-container">
            {blocks.into_iter().map(render_block).collect_view()}
        </div>
    }
    .into_any()
}

fn extract_blocks(input: &serde_json::Value) -> Vec<serde_json::Value> {
    let src = if let Some(obj) = input.as_object() {
        obj.get("blocks").cloned().unwrap_or_default()
    } else {
        input.clone()
    };
    src.as_array().cloned().unwrap_or_default()
}

fn render_block(block: serde_json::Value) -> impl IntoView {
    let block_type = block
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let data = block
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    match block_type.as_str() {
        "card" => blocks::render_card(data).into_any(),
        "table" => blocks::render_table(data).into_any(),
        "kv" => blocks::render_kv(data).into_any(),
        "status" => blocks::render_status(data).into_any(),
        "progress" => blocks::render_progress(data).into_any(),
        "alert" => blocks::render_alert(data).into_any(),
        "markdown" => blocks::render_markdown(data).into_any(),
        "button" => interactive::render_button(data).into_any(),
        "form" => interactive::render_form(data).into_any(),
        _ => view! {
            <div class="a2ui-unknown">
                {format!("Unknown block type: {}", block_type)}
            </div>
        }
        .into_any(),
    }
}
