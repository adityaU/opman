//! Dispatcher for document renderers — spreadsheet and document editors.
//! Thin module that routes to the appropriate editor component.

use leptos::prelude::*;

use crate::types::api::DocData;

use super::document_editor::render_document_editor;
use super::spreadsheet_editor::render_spreadsheet_editor;
use super::types::OpenFile;

/// Render the appropriate editable view for a document-type file.
pub fn render_doc_editor(file: &OpenFile, set_open_files: WriteSignal<Vec<OpenFile>>) -> AnyView {
    let data = match file.current_doc_data() {
        Some(d) => d,
        None => {
            return view! {
                <div class="file-preview file-preview-binary">
                    <span>"Loading..."</span>
                </div>
            }
            .into_any();
        }
    };

    match data {
        DocData::Spreadsheet { .. } => {
            render_spreadsheet_editor(file.path.clone(), data, set_open_files)
        }
        DocData::Document { .. } => render_document_editor(file.path.clone(), data, set_open_files),
        DocData::Presentation { .. } => view! {
            <div class="file-preview file-preview-binary">
                <span>"Presentation editing not yet supported"</span>
            </div>
        }
        .into_any(),
    }
}
