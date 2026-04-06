//! Sub-components for tool call rendering: ToolInput, ToolOutput, TodoList, EditDiffView.

use crate::components::code_block::{guess_language, CodeBlock};
use crate::components::icons::*;
use crate::components::message_turn::{parse_markdown_segments, ContentSegment};
use leptos::prelude::*;
use web_sys::HtmlElement;

pub use super::helpers::parse_output;
use super::helpers::ParsedOutput;

/// Render tool input as syntax-highlighted JSON via CodeBlock, or plain text.
#[component]
pub fn ToolInput(data: serde_json::Value) -> impl IntoView {
    let formatted = match &data {
        serde_json::Value::String(s) => s.clone(),
        _ => serde_json::to_string_pretty(&data).unwrap_or_default(),
    };
    let is_json = !matches!(&data, serde_json::Value::String(_));

    if is_json {
        view! {
            <div class="tool-call-input-block">
                <CodeBlock language="json".to_string() code=formatted />
            </div>
        }
        .into_any()
    } else {
        view! {
            <pre class="tool-call-pre">
                {formatted}
            </pre>
        }
        .into_any()
    }
}

/// Smart output rendering based on content format.
/// Matches React ToolOutput: auto-scroll for live output, markdown rendering,
/// file output with syntax highlighting.
#[component]
pub fn ToolOutput(
    #[prop(into)] output: String,
    #[prop(into)] tool_name: String,
    #[prop(optional)] is_live: bool,
) -> impl IntoView {
    let parsed = parse_output(&output);

    match parsed {
        ParsedOutput::File { path, content } => {
            let lang = guess_language(&path).to_string();
            view! {
                <div class="tool-output-file">
                    <div class="tool-output-file-header">
                        <span class="tool-output-file-path">{path}</span>
                    </div>
                    <CodeBlock language=lang code=content />
                </div>
            }
            .into_any()
        }
        ParsedOutput::Markdown { content } => {
            let segments = parse_markdown_segments(&content);
            view! {
                <div class="tool-output-markdown">
                    {segments.into_iter().map(|seg| {
                        match seg {
                            ContentSegment::Html(html) => {
                                view! {
                                    <div inner_html=html></div>
                                }.into_any()
                            }
                            ContentSegment::FencedCode { language, code } => {
                                view! {
                                    <CodeBlock language=language code=code />
                                }.into_any()
                            }
                        }
                    }).collect_view()}
                </div>
            }
            .into_any()
        }
        ParsedOutput::Plain { content } => {
            let live_ref = NodeRef::<leptos::html::Pre>::new();
            let class = if is_live {
                "tool-call-pre tool-call-live-output"
            } else {
                "tool-call-pre"
            };

            // Auto-scroll live output only if user is near bottom
            if is_live {
                let content_clone = content.clone();
                let live_ref_clone = live_ref.clone();
                Effect::new(move |_| {
                    let _ = &content_clone;
                    if let Some(el) = live_ref_clone.get() {
                        let el: &HtmlElement = &el;
                        let distance = el.scroll_height() - el.scroll_top() - el.client_height();
                        if distance < 80 || el.scroll_top() == 0 {
                            el.set_scroll_top(el.scroll_height());
                        }
                    }
                });
            }

            view! {
                <pre node_ref=live_ref class=class>{content}</pre>
            }
            .into_any()
        }
    }
}

/// Render todowrite input as a checklist.
#[component]
pub fn TodoList(input: serde_json::Value) -> impl IntoView {
    let todos: Vec<(String, String, Option<String>)> = {
        let items = if let Some(obj) = input.as_object() {
            obj.get("todos").cloned().unwrap_or(input.clone())
        } else {
            input.clone()
        };

        if let Some(arr) = items.as_array() {
            arr.iter()
                .filter_map(|item| {
                    let content = item.get("content")?.as_str()?.to_string();
                    let status = item.get("status")?.as_str()?.to_string();
                    let priority = item
                        .get("priority")
                        .and_then(|p| p.as_str())
                        .map(|s| s.to_string());
                    Some((content, status, priority))
                })
                .collect()
        } else {
            vec![]
        }
    };

    if todos.is_empty() {
        return view! {
            <pre class="tool-call-pre tool-call-empty">"No todos"</pre>
        }
        .into_any();
    }

    view! {
        <div class="todo-list-items">
            {todos.iter().map(|(content, status, priority)| {
                let checkbox_class = format!("todo-checkbox {}", status);
                let content_class = format!("todo-content {}", status);

                view! {
                    <div class="todo-item">
                        <span class=checkbox_class>
                            {match status.as_str() {
                                "completed" => view! {
                                    <IconCheck size=10 />
                                }.into_any(),
                                "in_progress" => view! {
                                    <IconCircleDot size=10 />
                                }.into_any(),
                                "cancelled" => view! {
                                    <IconMinus size=10 />
                                }.into_any(),
                                _ => view! {
                                    <IconCircle size=8 />
                                }.into_any(),
                            }}
                        </span>
                        <span class=content_class>{content.clone()}</span>
                        {priority.as_ref().map(|p| {
                            let priority_class = format!("todo-priority {}", p);
                            view! {
                                <span class=priority_class>
                                    {p.clone()}
                                </span>
                            }
                        })}
                    </div>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}

/// Render edit tool input as a diff view.
#[component]
pub fn EditDiffView(input: serde_json::Value) -> impl IntoView {
    let diff = {
        let data = if let Some(s) = input.as_str() {
            serde_json::from_str::<serde_json::Value>(s).unwrap_or(input.clone())
        } else {
            input.clone()
        };

        let file_path = data
            .get("filePath")
            .or_else(|| data.get("file_path"))
            .or_else(|| data.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let old_str = data
            .get("oldString")
            .or_else(|| data.get("old_string"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let new_str = data
            .get("newString")
            .or_else(|| data.get("new_string"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if old_str.is_empty() && new_str.is_empty() {
            None
        } else {
            Some((file_path, old_str, new_str))
        }
    };

    match diff {
        None => view! { <ToolInput data=input /> }.into_any(),
        Some((file_path, old_str, new_str)) => {
            let old_lines: Vec<String> = old_str.lines().map(|l| l.to_string()).collect();
            let new_lines: Vec<String> = new_str.lines().map(|l| l.to_string()).collect();
            let old_count = old_lines.len();
            let new_count = new_lines.len();

            view! {
                <div class="diff-view">
                    {(!file_path.is_empty()).then(|| view! {
                        <div class="diff-header">
                            <span class="diff-file-path">{file_path}</span>
                        </div>
                    })}
                    {old_lines.iter().enumerate().map(|(i, line)| {
                        view! {
                            <div class="diff-line removed">
                                <span class="diff-line-num">{i + 1}</span>
                                <span class="diff-line-content">{format!("- {}", line)}</span>
                            </div>
                        }
                    }).collect_view()}
                    {new_lines.iter().enumerate().map(|(i, line)| {
                        view! {
                            <div class="diff-line added">
                                <span class="diff-line-num">{i + 1}</span>
                                <span class="diff-line-content">{format!("+ {}", line)}</span>
                            </div>
                        }
                    }).collect_view()}
                    <div class="diff-stats">
                        <span class="diff-stats-removed">
                            <IconMinus size=10 />
                            {format!(" {} removed", old_count)}
                        </span>
                        <span class="diff-stats-added">
                            <IconPlus size=10 />
                            {format!(" {} added", new_count)}
                        </span>
                    </div>
                </div>
            }
            .into_any()
        }
    }
}
