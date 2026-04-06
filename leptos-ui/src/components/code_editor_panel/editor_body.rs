//! Editor body view — matches React EditorBody.tsx.
//! Dispatches to the correct renderer based on file type and view mode.
//!
//! IMPORTANT: NativeEditor instances are rendered via `<For>` keyed by file
//! path and toggled with `style:display`. This prevents the editor from being
//! destroyed/recreated (losing buffer, cursor, undo history) when the user
//! switches tabs or clicks inside the panel.

use leptos::prelude::*;

use crate::components::icons::{IconFile, IconLoader2, IconX};

use super::file_renderers::render_native_editor;
use super::state::EditorState;
use super::types::{EditorViewMode, FileRenderType, OpenFile};

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

            // Loading overlay — separate from content so toggling editor_loading
            // does NOT destroy/recreate the NativeEditor underneath.
            {move || editor_loading.get().then(|| view! {
                <div class="code-editor-loading">
                    <IconLoader2 size=20 class="spin" />
                    <span>"Loading..."</span>
                </div>
            })}

            // Empty state — shown only when no files are open.
            // Tracks open_files (not active_file) so switching tabs won't toggle this.
            <div
                class="code-editor-empty-state"
                style:display=move || {
                    if open_files.get().is_empty() { "" } else { "none" }
                }
            >
                <IconFile size=32 />
                <span>"Select a file to edit"</span>
            </div>

            // Persistent NativeEditor instances — one per open text-editable file.
            // Keyed by path so they are created once and never destroyed on tab switch.
            // Visibility is toggled via style:display based on active_file + view mode.
            // Includes previewable types (Markdown, Html, Mermaid, Svg) so they get a
            // code editor for the "Code" view alongside the "Rendered" preview.
            <For
                each=move || {
                    use super::types::is_previewable_render_type;
                    let files = open_files.get();
                    let editable: Vec<_> = files.into_iter()
                        .filter(|f| f.render_type == FileRenderType::Code
                            || is_previewable_render_type(&f.render_type))
                        .map(|f| (f.path.clone(), f))
                        .collect();
                    editable
                }
                key=|(path, _)| path.clone()
                children=move |(path, file): (String, OpenFile)| {
                    let path_vis = path.clone();
                    let editor_view = render_native_editor(
                        &file, set_open_files, set_cursor_line, set_cursor_col,
                        pending_jump_line, set_pending_jump_line,
                    );
                    view! {
                        <div
                            class="editor-file-container"
                            style="height:100%;"
                            style:display=move || {
                                let af = active_file.get();
                                let vm = active_view.get();
                                if af.as_deref() == Some(path_vis.as_str())
                                    && vm == EditorViewMode::Code
                                {
                                    ""
                                } else {
                                    "none"
                                }
                            }
                        >
                            {editor_view}
                        </div>
                    }
                }
            />

            // Non-code file preview — re-evaluated when active_file / view_mode changes.
            // These are stateless renderers (images, markdown, etc.) so recreation is fine.
            // Document/Spreadsheet types get the editable renderer that can write back edits.
            {move || {
                let af = active_file.get();
                let view_mode = active_view.get();
                let files = open_files.get_untracked();
                let af = match af {
                    Some(p) => p,
                    None => return None,
                };
                let file = match files.iter().find(|f| f.path == af) {
                    Some(f) => f,
                    None => return None,
                };
                let rt = &file.render_type;
                // Code files are handled by the <For> above
                if *rt == FileRenderType::Code && view_mode == EditorViewMode::Code {
                    return None;
                }
                let content = file.current_content().to_string();
                Some(render_preview(file, &content, view_mode, set_open_files, set_mermaid_svg, set_mermaid_error, mermaid_svg, mermaid_error))
            }}
        </div>
    }
}

/// Render a non-code preview for the given file.
fn render_preview(
    file: &OpenFile,
    content: &str,
    view_mode: EditorViewMode,
    set_open_files: WriteSignal<Vec<OpenFile>>,
    set_mermaid_svg: WriteSignal<String>,
    set_mermaid_error: WriteSignal<Option<String>>,
    mermaid_svg: ReadSignal<String>,
    mermaid_error: ReadSignal<Option<String>>,
) -> AnyView {
    use super::doc_renderers::render_doc_editor;
    use super::file_renderers::{
        render_csv_view, render_html_view, render_markdown_view, render_mermaid_view,
        render_svg_view,
    };
    use super::types::is_previewable_render_type;

    let rt = &file.render_type;

    // Previewable types in Code mode use NativeEditor (rendered by <For> above)
    if is_previewable_render_type(rt) && view_mode == EditorViewMode::Code {
        return None::<AnyView>.into_any();
    }

    match rt {
        FileRenderType::Model3D => super::model3d_preview::render_model3d_view(&file.path),
        FileRenderType::Image => {
            let url = crate::api::files::file_raw_url(&file.path);
            super::image_preview::render_image_preview(&url, &file.path)
        }
        FileRenderType::Audio => {
            let url = crate::api::files::file_raw_url(&file.path);
            let name = file
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&file.name)
                .to_string();
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
            let name = file
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&file.name)
                .to_string();
            view! {
                <div class="file-preview file-preview-binary">
                    <IconFile size=48 />
                    <span class="file-preview-label">"Binary file \u{2014} cannot be displayed"</span>
                    <span class="file-preview-name">{name}</span>
                </div>
            }.into_any()
        }
        FileRenderType::Csv => render_csv_view(content),
        FileRenderType::Spreadsheet | FileRenderType::Document => {
            render_doc_editor(file, set_open_files)
        }
        FileRenderType::Markdown => render_markdown_view(content),
        FileRenderType::Html => render_html_view(content),
        FileRenderType::Mermaid => render_mermaid_view(
            content,
            set_mermaid_svg,
            set_mermaid_error,
            mermaid_svg,
            mermaid_error,
        ),
        FileRenderType::Svg => render_svg_view(content),
        FileRenderType::Code => {
            // Shouldn't reach here (handled by <For>), but fallback
            view! { <div /> }.into_any()
        }
    }
}
