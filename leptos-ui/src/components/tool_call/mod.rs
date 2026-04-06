//! ToolCall — accordion-style tool call rendering with status, duration, input/output.

pub mod a2ui;
pub mod bash_output;
pub mod bash_view;
pub mod helpers;
pub mod sub_components;
pub mod task_render;
pub mod todo_view;

use a2ui::A2uiBlocks;
use bash_view::render_bash_tool;
use helpers::auto_expand_default;
pub use helpers::{
    format_duration, format_tool_name, get_task_session_id, ChildSessionRef, SubagentMessagesMap,
};
use sub_components::{EditDiffView, ToolInput, ToolOutput};
use task_render::render_task_tool;
use todo_view::render_todo_tool;

use crate::components::icons::*;
use crate::components::message_timeline::AccordionState;
use crate::hooks::use_auto_open::{AutoOpenState, ToolCategory};
use crate::types::core::MessagePart;
use leptos::prelude::*;

// ── ToolCall Component ──────────────────────────────────────────────

#[component]
pub fn ToolCallView(
    part: MessagePart,
    child_session: Option<ChildSessionRef>,
    subagent_messages: Option<ReadSignal<SubagentMessagesMap>>,
    on_open_session: Option<Callback<String>>,
) -> impl IntoView {
    let tool_name = part
        .tool
        .clone()
        .or_else(|| part.tool_name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let short_name = format_tool_name(&tool_name);
    let is_todo_write = tool_name.contains("todowrite") || tool_name.contains("todo_write");
    let is_task_tool = tool_name == "task";
    let is_bash_tool =
        tool_name.contains("bash") || tool_name.contains("shell") || tool_name.contains("terminal");
    let is_edit_tool = tool_name.contains("edit") && !tool_name.contains("neovim");
    let is_a2ui = tool_name == "ui_render" || tool_name == "ui_ui_render";
    let category = ToolCategory::classify(&tool_name);
    let status = part
        .state
        .as_ref()
        .and_then(|s| s.status.as_deref())
        .unwrap_or("pending");
    let is_error = status == "error";
    let is_completed = status == "completed";
    let is_running = status == "running" || status == "pending";
    let task_session_id = if is_task_tool {
        get_task_session_id(&part, child_session.as_ref())
    } else {
        None
    };
    let has_subagent_messages = is_task_tool
        && task_session_id.as_ref().map_or(false, |sid| {
            subagent_messages.map_or(false, |sig| {
                sig.with_untracked(|m| m.get(sid).map_or(false, |msgs| !msgs.is_empty()))
            })
        });
    let duration_ms = part.state.as_ref().and_then(|s| {
        let time = s.time.as_ref()?;
        Some(time.end? - time.start?)
    });
    let input_data = part.state.as_ref().and_then(|s| s.input.clone());
    let has_input = input_data.as_ref().map_or(false, |d| match d {
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Object(m) => !m.is_empty(),
        _ => false,
    });
    let final_output = part.state.as_ref().and_then(|s| s.output.clone());
    let live_output = part
        .state
        .as_ref()
        .and_then(|s| s.metadata.as_ref())
        .and_then(|m| m.get("output"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let output_data = final_output.filter(|s| !s.is_empty()).or(live_output);
    let has_output = output_data.as_ref().map_or(false, |s| !s.is_empty());
    let error_text = if is_error {
        part.state
            .as_ref()
            .and_then(|s| s.error.clone())
            .or_else(|| (!has_output).then(|| "Tool call failed".to_string()))
    } else {
        None
    };
    let is_truncated = part
        .state
        .as_ref()
        .and_then(|s| s.metadata.as_ref())
        .and_then(|m| m.get("truncated"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let auto_open_config = use_context::<AutoOpenState>()
        .map(|s| s.get())
        .unwrap_or_default();

    let initial_expanded = auto_expand_default(
        category,
        is_running,
        is_completed,
        is_error,
        has_subagent_messages,
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
                // Seed so re-renders preserve initial state
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
    let ak_for_toggle = accordion_key.clone();
    let handle_toggle = move |_: web_sys::MouseEvent| {
        let new_val = !expanded.get_untracked();
        set_expanded.set(new_val);
        if let Some(AccordionState(map)) = accordion_ctx {
            map.update(|m| {
                m.insert(ak_for_toggle.clone(), new_val);
            });
        }
    };

    let title = part.state.as_ref().and_then(|s| s.title.clone());
    let child_title = child_session
        .as_ref()
        .map(|cs| cs.title.clone())
        .unwrap_or_else(|| "Task".to_string());
    if is_task_tool {
        return render_task_tool(
            task_session_id,
            subagent_messages,
            on_open_session,
            error_text.clone(),
            output_data.clone(),
            tool_name.clone(),
            title.unwrap_or(child_title),
            is_error,
            is_running,
            is_completed,
            has_output,
            is_truncated,
        );
    }

    // A2UI renders directly — no accordion wrapper
    if is_a2ui && has_input {
        let inp = input_data.clone().unwrap_or(serde_json::Value::Null);
        return view! { <A2uiBlocks input=inp /> }.into_any();
    }
    // A2UI loading — show inline pulse, not an accordion
    if is_a2ui && is_running {
        return view! {
            <div class="a2ui-loading">
                <span class="tool-pulse-dot" />
                " Rendering..."
            </div>
        }
        .into_any();
    }

    // Bash tools render with a dedicated non-accordion layout
    if is_bash_tool {
        return render_bash_tool(&part);
    }

    // Todowrite renders with its own collapsible accordion
    if is_todo_write {
        return render_todo_tool(&part);
    }

    let wrapper_class = if is_error {
        "tool-call tool-call-error"
    } else {
        "tool-call"
    };
    let status_str = status.to_string();
    let error_text_body = error_text.clone();
    let output_data_body = output_data.clone();
    let tool_name_body = tool_name.clone();

    view! {
        <div class=wrapper_class>
            <button class="tool-call-header" on:click=handle_toggle>
                <span class="tool-call-icon">
                    {move || if expanded.get() {
                        view! { <IconChevronDown size=14 /> }.into_any()
                    } else {
                        view! { <IconChevronRight size=14 /> }.into_any()
                    }}
                </span>
                <IconWrench size=12 />
                <span class="tool-call-name">{short_name}</span>
                {title.clone().map(|t| view! { <span class="tool-call-title">{t}</span> })}
                <span class="tool-call-status">
                    {duration_ms.map(|d| {
                        let formatted = format_duration(d);
                        view! {
                            <span class="tool-call-duration">
                                <IconClock size=10 />
                                {formatted}
                            </span>
                        }
                    })}
                    {if is_completed {
                        view! { <IconCheckCircle2 size=12 class="tool-success-icon" /> }.into_any()
                    } else if is_error {
                        view! { <IconXCircle size=12 class="tool-error-icon" /> }.into_any()
                    } else if is_running {
                        view! {
                            <span class="tool-call-pending">
                                <span class="tool-pulse-dot" />
                                " running"
                            </span>
                        }.into_any()
                    } else {
                        view! { <span class="tool-call-pending">{status_str.clone()}</span> }.into_any()
                    }}
                </span>
            </button>

            {move || expanded.get().then(|| {
                let input = input_data.clone();
                let output = output_data_body.clone();
                let err = error_text_body.clone();
                let tn = tool_name_body.clone();

                view! {
                    <div class="tool-call-body">
                        {(has_input).then(|| {
                            let inp = input.clone().unwrap_or(serde_json::Value::Null);
                            view! {
                                <div class="tool-call-section">
                                    <div class="tool-call-section-label">"Input"</div>
                                    {if is_edit_tool {
                                        view! { <EditDiffView input=inp /> }.into_any()
                                    } else {
                                        view! { <ToolInput data=inp /> }.into_any()
                                    }}
                                </div>
                            }
                        })}
                        {has_output.then(|| {
                            let out = output.clone().unwrap_or_default();
                            let tool = tn.clone();
                            view! {
                                <div class="tool-call-section">
                                    <div class="tool-call-section-label">"Output"</div>
                                    {is_truncated.then(|| view! {
                                        <span class="tool-call-truncated">"[truncated] "</span>
                                    })}
                                    <ToolOutput output=out tool_name=tool is_live=is_running />
                                </div>
                            }
                        })}
                        {err.map(|e| view! {
                            <div class="tool-call-section">
                                <div class="tool-call-error-banner">
                                    <IconAlertTriangle size=12 />
                                    <span>{e}</span>
                                </div>
                            </div>
                        })}
                        {(!has_output && error_text.is_none() && is_running).then(|| view! {
                            <div class="tool-call-section">
                                <div class="tool-call-section-label">"Output"</div>
                                <pre class="tool-call-pre tool-call-live-output">
                                    <span class="tool-pulse-dot" />
                                    " Waiting for output..."
                                </pre>
                            </div>
                        })}
                        {(!has_input && !has_output).then(|| view! {
                            <div class="tool-call-section">
                                <pre class="tool-call-pre tool-call-empty">"No data available"</pre>
                            </div>
                        })}
                    </div>
                }
            })}
        </div>
    }.into_any()
}
