//! Inline task-tool renderer (no accordion wrapper).

use crate::components::icons::*;
use crate::components::subagent_session::SubagentSession;
use leptos::prelude::*;

use super::helpers::SubagentMessagesMap;
use super::sub_components::ToolOutput;

/// Render task tool inline (without accordion wrapper).
#[allow(clippy::too_many_arguments)]
pub fn render_task_tool(
    task_session_id: Option<String>,
    subagent_messages: Option<SubagentMessagesMap>,
    on_open_session: Option<Callback<String>>,
    error_text: Option<String>,
    output_data: Option<String>,
    tool_name: String,
    sess_title: String,
    is_error: bool,
    is_running: bool,
    is_completed: bool,
    has_output: bool,
    is_truncated: bool,
) -> leptos::prelude::AnyView {
    let task_class = if is_error {
        "tool-call tool-call-task-inline tool-call-error"
    } else {
        "tool-call tool-call-task-inline"
    };

    view! {
        <div class=task_class>
            {if let Some(sid) = task_session_id {
                let msgs = subagent_messages.as_ref()
                    .and_then(|m| m.get(&sid)).cloned().unwrap_or_default();
                let on_open = on_open_session.clone();
                view! {
                    <SubagentSession
                        session_id=sid
                        title=sess_title
                        messages=msgs
                        is_running=is_running
                        is_completed=is_completed
                        is_error=is_error
                        on_open_session=on_open
                    />
                }.into_any()
            } else {
                view! {
                    <div>
                        {has_output.then(|| {
                            let out = output_data.clone().unwrap_or_default();
                            let tool = tool_name.clone();
                            view! {
                                <div class="tool-call-body">
                                    <div class="tool-call-section">
                                        <div class="tool-call-section-label">"Output"</div>
                                        {is_truncated.then(|| view! {
                                            <span class="tool-call-truncated">"[truncated] "</span>
                                        })}
                                        <ToolOutput output=out tool_name=tool is_live=is_running />
                                    </div>
                                </div>
                            }
                        })}
                        {error_text.clone().map(|e| view! {
                            <div class="tool-call-body">
                                <div class="tool-call-section">
                                    <div class="tool-call-error-banner">
                                        <IconAlertTriangle size=12 />
                                        <span>{e}</span>
                                    </div>
                                </div>
                            </div>
                        })}
                        {(!has_output && error_text.is_none() && is_running).then(|| view! {
                            <div class="tool-call-body">
                                <div class="tool-call-section">
                                    <div class="tool-call-section-label">"Output"</div>
                                    <pre class="tool-call-pre tool-call-live-output">
                                        <span class="tool-pulse-dot" />
                                        " Waiting for output..."
                                    </pre>
                                </div>
                            </div>
                        })}
                    </div>
                }.into_any()
            }}
        </div>
    }
    .into_any()
}
