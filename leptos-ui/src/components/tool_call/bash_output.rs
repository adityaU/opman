//! Terminal-styled bash tool output component.
//! Renders bash/shell tool calls with a terminal-like appearance:
//! - Shell bar with prompt icon and command
//! - Terminal body for output with auto-scroll when live

use crate::components::icons::*;
use leptos::prelude::*;
use web_sys::HtmlElement;

/// Extract command and description from bash tool input JSON.
pub fn extract_bash_info(input: &serde_json::Value) -> (String, Option<String>) {
    let obj = match input.as_object() {
        Some(o) => o,
        None => {
            if let Some(s) = input.as_str() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                    return extract_bash_info(&parsed);
                }
                return (s.to_string(), None);
            }
            return (String::new(), None);
        }
    };

    let command = obj
        .get("command")
        .or_else(|| obj.get("cmd"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let description = obj
        .get("description")
        .or_else(|| obj.get("desc"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    (command, description)
}

/// Terminal-styled output for bash/shell tool calls.
#[component]
pub fn BashTerminalOutput(
    #[prop(into)] command: String,
    description: Option<String>,
    output: Option<String>,
    #[prop(optional)] is_live: bool,
    #[prop(optional)] is_error: bool,
    error_text: Option<String>,
) -> impl IntoView {
    let has_output = output.as_ref().map_or(false, |s| !s.is_empty());
    let live_ref = NodeRef::<leptos::html::Pre>::new();

    // Auto-scroll live output only if user is already near the bottom.
    // This prevents resetting user's scroll position when new output arrives.
    if is_live && has_output {
        let output_clone = output.clone();
        let live_ref_clone = live_ref.clone();
        Effect::new(move |_| {
            let _ = &output_clone;
            if let Some(el) = live_ref_clone.get() {
                let el: &HtmlElement = &el;
                let distance = el.scroll_height() - el.scroll_top() - el.client_height();
                // Only auto-scroll if within 80px of the bottom (or first render)
                if distance < 80 || el.scroll_top() == 0 {
                    el.set_scroll_top(el.scroll_height());
                }
            }
        });
    }

    let body_class = if is_live {
        "bash-terminal-body bash-terminal-live"
    } else {
        "bash-terminal-body"
    };

    view! {
        <div class="bash-terminal">
            // Shell bar — description (small) above command, stacked vertically
            <div class="bash-terminal-bar">
                {description.map(|d| view! {
                    <span class="bash-terminal-desc">{d}</span>
                })}
                <pre class="bash-terminal-cmd">{command}</pre>
            </div>

            // Terminal body
            {if has_output {
                let content = output.clone().unwrap_or_default();
                view! {
                    <pre node_ref=live_ref class=body_class>{content}</pre>
                }.into_any()
            } else if is_live {
                view! {
                    <pre class=body_class>
                        <span class="tool-pulse-dot" />
                        " Running..."
                    </pre>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            // Error banner
            {error_text.map(|e| view! {
                <div class="bash-terminal-error">
                    <IconAlertTriangle size=11 />
                    <span>{e}</span>
                </div>
            })}
        </div>
    }
}
