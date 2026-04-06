//! Dedicated bash tool view — renders outside the normal accordion wrapper.
//! Shows: compact header row (terminal icon + description + duration + icon-only status),
//! always-visible command bar, and click-to-toggle output pane.

use crate::components::icons::*;
use crate::components::message_timeline::AccordionState;
use crate::components::tool_call::bash_output::{extract_bash_info, BashTerminalOutput};
use crate::components::tool_call::helpers::format_duration;
use crate::hooks::use_auto_open::{AutoOpenState, ToolCategory};
use crate::types::core::MessagePart;
use leptos::prelude::*;

/// Render a bash/shell/terminal tool call with a non-accordion layout.
/// Called from `ToolCallView` as an early return (like A2UI / task tool).
pub fn render_bash_tool(part: &MessagePart) -> leptos::prelude::AnyView {
    let status = part
        .state
        .as_ref()
        .and_then(|s| s.status.as_deref())
        .unwrap_or("pending");
    let is_error = status == "error";
    let is_completed = status == "completed";
    let is_running = status == "running" || status == "pending";

    let duration_ms = part.state.as_ref().and_then(|s| {
        let time = s.time.as_ref()?;
        Some(time.end? - time.start?)
    });

    let input_data = part.state.as_ref().and_then(|s| s.input.clone());
    let inp = input_data.unwrap_or(serde_json::Value::Null);
    let (command, description) = extract_bash_info(&inp);

    let final_output = part.state.as_ref().and_then(|s| s.output.clone());
    let live_output = part
        .state
        .as_ref()
        .and_then(|s| s.metadata.as_ref())
        .and_then(|m| m.get("output"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let output_data = final_output.filter(|s| !s.is_empty()).or(live_output);

    let error_text = if is_error {
        part.state
            .as_ref()
            .and_then(|s| s.error.clone())
            .or_else(|| {
                let has_out = output_data.as_ref().map_or(false, |s| !s.is_empty());
                (!has_out).then(|| "Tool call failed".to_string())
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

    // Output visibility toggle — auto-expand while running or if user config says so
    let auto_open_bash = use_context::<AutoOpenState>()
        .map(|s| s.category(ToolCategory::Bash))
        .unwrap_or(false);
    let initial_show = is_running || auto_open_bash;
    let accordion_key = part
        .tool_call_id
        .clone()
        .or_else(|| part.call_id.clone())
        .or_else(|| part.id.clone())
        .unwrap_or_default();

    let accordion_ctx = use_context::<AccordionState>();
    let default_show = if let Some(AccordionState(map)) = accordion_ctx {
        let saved = map.with_untracked(|m| m.get(&accordion_key).copied());
        if let Some(val) = saved {
            val
        } else {
            map.update_untracked(|m| {
                m.insert(accordion_key.clone(), initial_show);
            });
            initial_show
        }
    } else {
        initial_show
    };

    let (show_output, set_show_output) = signal(default_show);
    let ak = accordion_key.clone();
    let toggle_output = move |_: web_sys::MouseEvent| {
        let new_val = !show_output.get_untracked();
        set_show_output.set(new_val);
        if let Some(AccordionState(map)) = accordion_ctx {
            map.update(|m| {
                m.insert(ak.clone(), new_val);
            });
        }
    };

    let desc_for_header = description.clone();
    let wrapper_class = if is_error {
        "bash-view bash-view-error"
    } else {
        "bash-view"
    };

    view! {
        <div class=wrapper_class>
            // Compact header: terminal icon + description + duration + status icon
            <div class="bash-view-header">
                <IconTerminal size=12 />
                {desc_for_header.map(|d| view! {
                    <span class="bash-view-desc">{d}</span>
                })}
                <span class="bash-view-status">
                    {duration_ms.map(|d| {
                        let formatted = format_duration(d);
                        view! {
                            <span class="bash-view-duration">
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
                        view! { <span class="tool-pulse-dot" /> }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                </span>
            </div>

            // Command bar — always visible, click toggles output
            <button class="bash-view-cmd-bar" on:click=toggle_output>
                <span class="bash-view-chevron">
                    {move || if show_output.get() {
                        view! { <IconChevronDown size=12 /> }.into_any()
                    } else {
                        view! { <IconChevronRight size=12 /> }.into_any()
                    }}
                </span>
                <pre class="bash-terminal-cmd">{command.clone()}</pre>
            </button>

            // Toggleable output pane
            {move || show_output.get().then(|| {
                let out = output_data.clone();
                let err = error_text.clone();
                let no_desc: Option<String> = None;
                view! {
                    <div class="bash-view-output">
                        {is_truncated.then(|| view! {
                            <span class="tool-call-truncated">"[truncated] "</span>
                        })}
                        <BashTerminalOutput
                            command=String::new()
                            description=no_desc
                            output=out
                            is_live=is_running
                            is_error=is_error
                            error_text=err
                        />
                    </div>
                }
            })}
        </div>
    }
    .into_any()
}
