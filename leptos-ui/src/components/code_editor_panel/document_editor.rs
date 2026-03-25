//! Editable document (docx) component — contenteditable HTML editing with
//! a formatting toolbar for bold, italic, underline, headings, and lists.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icons::IconFile;
use crate::types::api::DocData;

use super::types::OpenFile;

#[wasm_bindgen(inline_js = "export function exec_cmd(c, v) { document.execCommand(c, false, v); }")]
extern "C" {
    fn exec_cmd(cmd: &str, value: &str);
}

/// Execute a `document.execCommand` formatting action.
fn exec_command(cmd: &str, value: &str) {
    exec_cmd(cmd, value);
}

/// Render an editable document viewer with formatting toolbar.
pub fn render_document_editor(
    path: String,
    data: &DocData,
    set_open_files: WriteSignal<Vec<OpenFile>>,
) -> AnyView {
    let html = match data {
        DocData::Document { html } => html.clone(),
        _ => {
            return view! {
                <div class="file-preview file-preview-binary">
                    <span>"Not a document"</span>
                </div>
            }
            .into_any();
        }
    };

    if html.trim().is_empty() {
        return view! {
            <div class="file-preview file-preview-binary">
                <IconFile size=48 />
                <span>"Empty document"</span>
            </div>
        }
        .into_any();
    }

    let file_path = path.clone();

    view! {
        <div class="document-viewer document-viewer-editable">
            <div class="document-toolbar">
                <button class="doc-toolbar-btn" title="Bold (Ctrl+B)"
                    on:click=move |_| exec_command("bold", "")
                >"B"</button>
                <button class="doc-toolbar-btn doc-toolbar-italic" title="Italic (Ctrl+I)"
                    on:click=move |_| exec_command("italic", "")
                >"I"</button>
                <button class="doc-toolbar-btn doc-toolbar-underline" title="Underline (Ctrl+U)"
                    on:click=move |_| exec_command("underline", "")
                >"U"</button>
                <button class="doc-toolbar-btn doc-toolbar-strike" title="Strikethrough"
                    on:click=move |_| exec_command("strikeThrough", "")
                >"S"</button>
                <span class="doc-toolbar-sep" />
                <button class="doc-toolbar-btn" title="Heading 1"
                    on:click=move |_| exec_command("formatBlock", "h1")
                >"H1"</button>
                <button class="doc-toolbar-btn" title="Heading 2"
                    on:click=move |_| exec_command("formatBlock", "h2")
                >"H2"</button>
                <button class="doc-toolbar-btn" title="Heading 3"
                    on:click=move |_| exec_command("formatBlock", "h3")
                >"H3"</button>
                <button class="doc-toolbar-btn" title="Paragraph"
                    on:click=move |_| exec_command("formatBlock", "p")
                >"P"</button>
                <span class="doc-toolbar-sep" />
                <button class="doc-toolbar-btn" title="Bulleted list"
                    on:click=move |_| exec_command("insertUnorderedList", "")
                >"UL"</button>
                <button class="doc-toolbar-btn" title="Numbered list"
                    on:click=move |_| exec_command("insertOrderedList", "")
                >"OL"</button>
            </div>
            <div class="document-content"
                contenteditable="true"
                inner_html=html
                on:input=move |ev| {
                    let el = ev.target().unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();
                    let new_html = el.inner_html();
                    let p = file_path.clone();
                    set_open_files.update(|fs| {
                        if let Some(f) = fs.iter_mut().find(|f| f.path == p) {
                            f.edited_doc_data = Some(DocData::Document { html: new_html });
                        }
                    });
                }
            />
        </div>
    }
    .into_any()
}
