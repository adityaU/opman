//! Editor toolbar view — matches React EditorToolbar.tsx.

use leptos::prelude::*;

use crate::components::icons::{
    IconAlertCircle, IconArrowRightCircle, IconChevronLeft, IconCode2, IconEye, IconInfo,
    IconLoader2, IconRotateCcw, IconSave, IconWand2,
};

use super::state::EditorState;
use super::types::{is_binary_render_type, is_previewable_render_type, EditorViewMode};

/// Render the editor toolbar for the active file.
/// When `is_mobile` is true and `on_back` is provided, shows a back button.
pub fn render_editor_toolbar(
    s: &EditorState,
    active_view: Memo<EditorViewMode>,
    save_file: std::rc::Rc<dyn Fn(String)>,
    revert_file: std::rc::Rc<dyn Fn(String)>,
    handle_hover: std::rc::Rc<dyn Fn()>,
    handle_definition: std::rc::Rc<dyn Fn()>,
    handle_format: std::rc::Rc<dyn Fn()>,
    set_active_view: std::rc::Rc<dyn Fn(EditorViewMode)>,
    is_mobile: ReadSignal<bool>,
    on_back: Option<std::rc::Rc<dyn Fn()>>,
) -> impl IntoView {
    let active_file = s.active_file;
    let open_files = s.open_files;
    let diagnostics = s.diagnostics;
    let lsp_available = s.lsp_available;
    let lsp_busy = s.lsp_busy;
    let save_status = s.save_status;
    let saving = s.saving;

    let save_file = send_wrapper::SendWrapper::new(save_file);
    let revert_file = send_wrapper::SendWrapper::new(revert_file);
    let sav = send_wrapper::SendWrapper::new(set_active_view.clone());
    let sav2 = send_wrapper::SendWrapper::new(set_active_view);
    let hh = send_wrapper::SendWrapper::new(handle_hover);
    let hd = send_wrapper::SendWrapper::new(handle_definition);
    let hf = send_wrapper::SendWrapper::new(handle_format);
    let on_back = on_back.map(send_wrapper::SendWrapper::new);

    move || {
        let af = active_file.get();
        let files = open_files.get();
        let current = af
            .as_ref()
            .and_then(|p| files.iter().find(|f| &f.path == p));
        let Some(f) = current else {
            return None;
        };

        let path_save = f.path.clone();
        let path_revert = f.path.clone();
        let modified = f.is_modified();
        let rt = f.render_type.clone();
        let is_prev = is_previewable_render_type(&rt);
        let is_bin = is_binary_render_type(&rt);
        let file_path_display = f.path.clone();
        let file_path_title = file_path_display.clone();
        let save_file = save_file.clone();
        let revert_file = revert_file.clone();
        let sav_c = sav.clone();
        let sav_c2 = sav2.clone();
        let hh_c = hh.clone();
        let hd_c = hd.clone();
        let hf_c = hf.clone();
        let on_back_c = on_back.clone();

        Some(view! {
            <div class="code-editor-toolbar">
                {move || {
                    let mobile = is_mobile.get();
                    let back = on_back_c.clone();
                    (mobile && back.is_some()).then(move || {
                        let back = back.unwrap();
                        view! {
                            <button class="code-editor-back" title="Back to files" aria-label="Back to files"
                                on:click=move |_| back()
                            ><IconChevronLeft size=14 /></button>
                        }
                    })
                }}
                <span class="code-editor-filename" title=file_path_title>{file_path_display}</span>
                {modified.then(|| view! { <span class="code-editor-modified-dot" title="Unsaved changes">{"\u{2022}"}</span> })}
                <span class="code-editor-spacer" />

                {is_prev.then(|| view! {
                    <div class="code-editor-view-tabs">
                        <button
                            class=move || if active_view.get() == EditorViewMode::Code { "code-editor-view-tab active" } else { "code-editor-view-tab" }
                            on:click=move |_| sav_c(EditorViewMode::Code)
                        ><IconCode2 size=13 />" Code"</button>
                        <button
                            class=move || if active_view.get() == EditorViewMode::Rendered { "code-editor-view-tab active" } else { "code-editor-view-tab" }
                            on:click=move |_| sav_c2(EditorViewMode::Rendered)
                        ><IconEye size=13 />" Rendered"</button>
                    </div>
                })}

                {(!is_bin).then(|| view! {
                    <div class="code-editor-lsp-group">
                        <span class=move || if lsp_available.get() { "code-editor-lsp-pill active" } else { "code-editor-lsp-pill inactive" }>
                            <IconAlertCircle size=12 />
                            {move || format!(" {} issues", diagnostics.get().len())}
                        </span>
                        <button class="code-editor-action" title="Hover info at cursor"
                            on:click=move |_| hh_c()
                        >{move || if lsp_busy.get().as_deref() == Some("hover") {
                            view! { <IconLoader2 size=13 class="spin" /> }.into_any()
                        } else { view! { <IconInfo size=13 /> }.into_any() }}</button>
                        <button class="code-editor-action" title="Go to definition"
                            on:click=move |_| hd_c()
                        >{move || if lsp_busy.get().as_deref() == Some("definition") {
                            view! { <IconLoader2 size=13 class="spin" /> }.into_any()
                        } else { view! { <IconArrowRightCircle size=13 /> }.into_any() }}</button>
                        <button class="code-editor-action" title="Format with LSP"
                            on:click=move |_| hf_c()
                        >{move || if lsp_busy.get().as_deref() == Some("format") {
                            view! { <IconLoader2 size=13 class="spin" /> }.into_any()
                        } else { view! { <IconWand2 size=13 /> }.into_any() }}</button>
                    </div>
                })}

                {move || match save_status.get().as_deref() {
                    Some("saved") => Some(view! { <span class="code-editor-save-status">"Saved"</span> }.into_any()),
                    Some("saving") => Some(view! { <span class="code-editor-save-status"><IconLoader2 size=13 class="spin" /></span> }.into_any()),
                    _ => None,
                }}

                {modified.then(|| {
                    let save = save_file.clone();
                    let revert = revert_file.clone();
                    let ps = path_save.clone();
                    let pr = path_revert.clone();
                    view! {
                        <button class="code-editor-action" title="Revert changes" aria-label="Revert changes"
                            on:click=move |_| revert(pr.clone())
                        ><IconRotateCcw size=13 /></button>
                        <button class="code-editor-action code-editor-save" title="Save (Cmd+S)" aria-label="Save file"
                            disabled=move || saving.get()
                            on:click=move |_| save(ps.clone())
                        >{move || if saving.get() {
                            view! { <IconLoader2 size=13 class="spin" /> }.into_any()
                        } else { view! { <IconSave size=13 /> }.into_any() }}</button>
                    }
                })}
            </div>
        })
    }
}
