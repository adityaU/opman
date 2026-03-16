//! Preview renderers and native editor view for the code editor panel.
//! Matches React FileRenderers.tsx + EditorBody rendering.

use leptos::prelude::*;

use crate::components::code_block::CodeBlock;
use crate::components::icons::{IconFile, IconLoader2};
use crate::components::message_turn::{parse_markdown_segments, ContentSegment};

use super::js_helpers::{render_mermaid_js, sanitize_html, sanitize_svg};
use super::native_editor::NativeEditor;
use super::types::{language_to_extension, parse_csv, OpenFile};

/// Render CSV content as an HTML table (matches React CsvViewer).
pub fn render_csv_view(content: &str) -> AnyView {
    let rows = parse_csv(content);
    if rows.is_empty() {
        return view! {
            <div class="file-preview file-preview-binary">
                <span>"Empty CSV file"</span>
            </div>
        }
        .into_any();
    }

    let header = rows[0].clone();
    let body: Vec<Vec<String>> = rows[1..]
        .iter()
        .filter(|r| r.iter().any(|c| !c.is_empty()))
        .cloned()
        .collect();

    view! {
        <div class="csv-viewer">
            <table>
                <thead>
                    <tr>
                        {header.iter().map(|cell| {
                            view! { <th>{cell.clone()}</th> }
                        }).collect::<Vec<_>>()}
                    </tr>
                </thead>
                <tbody>
                    {body.iter().map(|row| {
                        view! {
                            <tr>
                                {row.iter().map(|cell| {
                                    view! { <td>{cell.clone()}</td> }
                                }).collect::<Vec<_>>()}
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }
    .into_any()
}

/// Render markdown content (matches React MarkdownViewer).
pub fn render_markdown_view(content: &str) -> AnyView {
    let segments = parse_markdown_segments(content);
    view! {
        <div class="markdown-viewer">
            {segments.into_iter().map(|seg| {
                match seg {
                    ContentSegment::Html(html) => {
                        view! { <div inner_html=html></div> }.into_any()
                    }
                    ContentSegment::FencedCode { language, code } => {
                        view! { <CodeBlock language=language code=code /> }.into_any()
                    }
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}

/// Render HTML content in a sandboxed iframe (matches React HtmlViewer).
pub fn render_html_view(content: &str) -> AnyView {
    let sanitized = sanitize_html(content);
    view! {
        <iframe
            class="html-viewer-frame"
            sandbox="allow-scripts allow-same-origin"
            srcdoc=sanitized
            title="HTML preview"
        />
    }
    .into_any()
}

/// Render SVG content with DOMPurify sanitization (matches React SvgViewer).
pub fn render_svg_view(content: &str) -> AnyView {
    let sanitized = sanitize_svg(content);
    view! { <div class="svg-viewer" inner_html=sanitized /> }.into_any()
}

/// Render Mermaid diagram (matches React MermaidViewer).
pub fn render_mermaid_view(
    content: &str,
    set_svg: WriteSignal<String>,
    set_error: WriteSignal<Option<String>>,
    svg: ReadSignal<String>,
    error: ReadSignal<Option<String>>,
) -> AnyView {
    let content_owned = content.to_string();
    leptos::task::spawn_local(async move {
        render_mermaid_js(&content_owned, set_svg, set_error);
    });

    view! {
        {move || {
            if let Some(err) = error.get() {
                return view! {
                    <div class="file-preview file-preview-binary"><span>{err}</span></div>
                }.into_any();
            }
            let s = svg.get();
            if s.is_empty() {
                view! {
                    <div class="code-editor-loading">
                        <IconLoader2 size=20 class="spin" />
                        <span>"Rendering diagram..."</span>
                    </div>
                }.into_any()
            } else {
                view! { <div class="mermaid-viewer" inner_html=s /> }.into_any()
            }
        }}
    }
    .into_any()
}

/// Render a code file using the Rust-native editor (replaces CodeMirror).
pub fn render_native_editor(
    file: &OpenFile,
    set_open_files: WriteSignal<Vec<OpenFile>>,
    set_cursor_line: WriteSignal<u32>,
    set_cursor_col: WriteSignal<u32>,
    pending_jump_line: ReadSignal<Option<u32>>,
    set_pending_jump_line: WriteSignal<Option<u32>>,
) -> AnyView {
    let content = file.current_content().to_string();
    let ext = language_to_extension(&file.language);
    let file_path = file.path.clone();

    // Create a signal from pending_jump_line that clears after use.
    let jump_signal = Signal::derive(move || {
        let val = pending_jump_line.get();
        if val.is_some() {
            set_pending_jump_line.set(None);
        }
        val
    });

    let on_change = {
        let path = file_path.clone();
        let original = file.content.clone();
        Callback::new(move |new_content: String| {
            let path = path.clone();
            let original = original.clone();
            set_open_files.update(|files| {
                if let Some(f) = files.iter_mut().find(|f| f.path == path) {
                    if new_content == original {
                        f.edited_content = None;
                    } else {
                        f.edited_content = Some(new_content);
                    }
                }
            });
        })
    };

    let on_cursor = Callback::new(move |(line, col): (u32, u32)| {
        set_cursor_line.set(line);
        set_cursor_col.set(col);
    });

    view! {
        <NativeEditor
            content=content
            extension=ext
            on_change=on_change
            on_cursor=on_cursor
            jump_to_line=jump_signal
        />
    }
    .into_any()
}
