//! ToolCall — accordion-style tool call rendering with status, duration, input/output.
//! Leptos port of `web-ui/src/tool-call/ToolCall.tsx` + `components.tsx`.
//! Matches React CSS classes exactly (tool-call, tool-call-header, tool-call-name, etc.)

pub mod helpers;
pub mod sub_components;
pub mod task_render;

pub use helpers::{
    format_duration, format_tool_name, get_task_session_id, ChildSessionRef, SubagentMessagesMap,
};
use sub_components::{EditDiffView, TodoList, ToolInput, ToolOutput};
use task_render::render_task_tool;

use crate::components::icons::*;
use crate::types::core::MessagePart;
use leptos::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

// ── Persistent expanded-state store ─────────────────────────────────
// WASM is single-threaded so thread_local + RefCell is safe and lock-free.
// Keyed by tool call_id / tool_call_id / id (first non-empty).
thread_local! {
    static EXPANDED_STATE: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
}

fn expanded_key(part: &MessagePart) -> Option<String> {
    part.call_id
        .clone()
        .or_else(|| part.tool_call_id.clone())
        .or_else(|| part.id.clone())
        .filter(|k| !k.is_empty())
}

fn get_persisted_expanded(key: &str) -> Option<bool> {
    EXPANDED_STATE.with(|m| m.borrow().get(key).copied())
}

fn set_persisted_expanded(key: &str, val: bool) {
    EXPANDED_STATE.with(|m| {
        m.borrow_mut().insert(key.to_string(), val);
    });
}

// ── ToolCall Component ──────────────────────────────────────────────

#[component]
pub fn ToolCallView(
    part: MessagePart,
    child_session: Option<ChildSessionRef>,
    subagent_messages: Option<SubagentMessagesMap>,
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

    let has_subagent_messages = if is_task_tool {
        if let (Some(sid), Some(ref msgs)) = (&task_session_id, &subagent_messages) {
            msgs.get(sid).map_or(false, |m| !m.is_empty())
        } else {
            false
        }
    } else {
        false
    };

    let duration_ms = part.state.as_ref().and_then(|s| {
        let time = s.time.as_ref()?;
        let start = time.start?;
        let end = time.end?;
        Some(end - start)
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
            .or_else(|| {
                if !has_output {
                    Some("Tool call failed".to_string())
                } else {
                    None
                }
            })
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

    let initial_expanded = is_todo_write
        || (is_task_tool && (is_running || is_completed || is_error || has_subagent_messages))
        || (is_bash_tool && is_running);

    // Persist expanded state across rerenders using a stable key.
    let stable_key = expanded_key(&part);
    let starting = match &stable_key {
        Some(k) => get_persisted_expanded(k).unwrap_or(initial_expanded),
        None => initial_expanded,
    };
    let (expanded, set_expanded) = signal(starting);

    let toggle_key = stable_key.clone();
    let handle_toggle = move |_: web_sys::MouseEvent| {
        set_expanded.update(|v| {
            *v = !*v;
            if let Some(ref k) = toggle_key {
                set_persisted_expanded(k, *v);
            }
        });
    };

    let title = part.state.as_ref().and_then(|s| s.title.clone());
    let child_title = child_session
        .as_ref()
        .map(|cs| cs.title.clone())
        .unwrap_or_else(|| "Task".to_string());

    // ── Task tool: render inline without accordion ──
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

    // ── Standard tool: accordion ──
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
                                <IconLoader2 size=12 class="tool-spin-icon" />
                                " running..."
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
                        {if is_todo_write && has_input {
                            view! {
                                <div class="tool-call-section">
                                    <div class="tool-call-section-label">"Todos"</div>
                                    <TodoList input=input.clone().unwrap_or(serde_json::Value::Null) />
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div>
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
                                                <IconLoader2 size=12 class="tool-spin-icon" />
                                                " Waiting for output..."
                                            </pre>
                                        </div>
                                    })}
                                </div>
                            }.into_any()
                        }}
                        {(!is_todo_write && !has_input && !has_output).then(|| view! {
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
