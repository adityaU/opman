//! Editor body view — matches React EditorBody.tsx.
//! Dispatches to the correct renderer based on file type and view mode.

use leptos::prelude::*;

use crate::components::icons::{IconFile, IconLoader2, IconX};

use super::file_renderers::{
    render_csv_view, render_html_view, render_markdown_view, render_mermaid_view,
    render_native_editor, render_svg_view,
};
use super::state::EditorState;
use super::types::{is_previewable_render_type, EditorViewMode, FileRenderType};

/// Render the editor body (hover card, diagnostics, file content).
pub fn render_editor_body(s: &EditorState, active_view: Memo<EditorViewMode>) -> impl IntoView {
    let active_file = s.active_file;
    let open_files = s.open_files;
    let editor_loading = s.editor_loading;
    let hover_text = s.hover_text;
    let set_hover_text = s.set_hover_text;
    let diagnostics = s.diagnostics;
    let set_open_files = s.set_open_files;
    let set_cursor_line = s.set_cursor_line;
    let set_cursor_col = s.set_cursor_col;
    let pending_jump_line = s.pending_jump_line;
    let set_pending_jump_line = s.set_pending_jump_line;
    let mermaid_svg = s.mermaid_svg;
    let set_mermaid_svg = s.set_mermaid_svg;
    let mermaid_error = s.mermaid_error;
    let set_mermaid_error = s.set_mermaid_error;

    view! {
        <div class="code-editor-body">
            // Hover card
            {move || hover_text.get().map(|text| view! {
                <div class="code-editor-hover-card">
                    <div class="code-editor-hover-title">"Hover"</div>
                    <pre>{text}</pre>
                    <button class="absolute top-1 right-1 text-text-muted hover:text-text text-xs"
                        on:click=move |_| set_hover_text.set(None)
                    ><IconX size=12 /></button>
                </div>
            })}

            // Diagnostics panel (filtered to active file)
            {move || {
                let diags = diagnostics.get();
                let af = active_file.get();
                let filtered: Vec<_> = diags.iter().filter(|d| {
                    af.as_ref().map(|ap| d.file.ends_with(ap.as_str()) || d.file.as_str() == ap.as_str()).unwrap_or(false)
                }).collect();
                if filtered.is_empty() { return None; }
                Some(view! {
                    <div class="code-editor-diagnostics">
                        {filtered.iter().take(6).map(|d| {
                            let sev = d.severity.to_lowercase();
                            let class = format!("code-editor-diagnostic severity-{sev}");
                            view! {
                                <div class=class>
                                    <span class="code-editor-diagnostic-pos">{format!("L{}:C{}", d.lnum, d.col)}</span>
                                    <span class="code-editor-diagnostic-msg">{d.message.clone()}</span>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                })
            }}

            // Main content area
            {move || {
                if editor_loading.get() {
                    return view! {
                        <div class="code-editor-loading">
                            <IconLoader2 size=20 class="spin" />
                            <span>"Loading..."</span>
                        </div>
                    }.into_any();
                }
                let af = active_file.get();
                let files = open_files.get();
                if af.is_none() || files.is_empty() {
                    return view! {
                        <div class="code-editor-empty-state">
                            <IconFile size=32 />
                            <span>"Select a file to edit"</span>
                        </div>
                    }.into_any();
                }
                let active_path = af.unwrap();
                let Some(file) = files.iter().find(|f| f.path == active_path) else {
                    return view! {
                        <div class="code-editor-empty-state"><span>"File not found"</span></div>
                    }.into_any();
                };

                let view_mode = active_view.get();
                let rt = &file.render_type;
                let content = file.current_content().to_string();

                if is_previewable_render_type(rt) && view_mode == EditorViewMode::Code {
                    return render_native_editor(
                        file, set_open_files, set_cursor_line, set_cursor_col,
                        pending_jump_line, set_pending_jump_line,
                    );
                }

                match rt {
                    FileRenderType::Image => {
                        let url = crate::api::files::file_raw_url(&file.path);
                        view! { <div class="file-preview file-preview-image"><img src=url alt=file.path.clone() /></div> }.into_any()
                    }
                    FileRenderType::Audio => {
                        let url = crate::api::files::file_raw_url(&file.path);
                        let name = file.path.rsplit('/').next().unwrap_or(&file.name).to_string();
                        view! {
                            <div class="file-preview file-preview-audio">
                                <div class="file-preview-icon"><IconFile size=48 /></div>
                                <span class="file-preview-name">{name}</span>
                                <audio controls=true src=url preload="metadata">"Your browser does not support the audio element."</audio>
                            </div>
                        }.into_any()
                    }
                    FileRenderType::Video => {
                        let url = crate::api::files::file_raw_url(&file.path);
                        view! {
                            <div class="file-preview file-preview-video">
                                <video controls=true src=url preload="metadata">"Your browser does not support the video element."</video>
                            </div>
                        }.into_any()
                    }
                    FileRenderType::Pdf => {
                        let url = crate::api::files::file_raw_url(&file.path);
                        let title = file.path.clone();
                        view! { <div class="file-preview file-preview-pdf"><iframe src=url title=title /></div> }.into_any()
                    }
                    FileRenderType::Binary => {
                        let name = file.path.rsplit('/').next().unwrap_or(&file.name).to_string();
                        view! {
                            <div class="file-preview file-preview-binary">
                                <IconFile size=48 />
                                <span class="file-preview-label">"Binary file \u{2014} cannot be displayed"</span>
                                <span class="file-preview-name">{name}</span>
                            </div>
                        }.into_any()
                    }
                    FileRenderType::Csv => render_csv_view(&content),
                    FileRenderType::Markdown => render_markdown_view(&content),
                    FileRenderType::Html => render_html_view(&content),
                    FileRenderType::Mermaid => render_mermaid_view(&content, set_mermaid_svg, set_mermaid_error, mermaid_svg, mermaid_error),
                    FileRenderType::Svg => render_svg_view(&content),
                    FileRenderType::Code => render_native_editor(
                        file, set_open_files, set_cursor_line, set_cursor_col,
                        pending_jump_line, set_pending_jump_line,
                    ),
                }
            }}
        </div>
    }
}
