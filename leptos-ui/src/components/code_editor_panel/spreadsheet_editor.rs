//! Editable spreadsheet component — inline cell editing, add/remove rows/columns/sheets.
//! Renders the active sheet as an HTML table with click-to-edit cells.

use std::rc::Rc;

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::JsCast;

use crate::types::api::{DocData, SheetData};

use super::types::OpenFile;

/// Shared sync callback wrapped for Send safety in Leptos view closures.
type SyncFn = SendWrapper<Rc<dyn Fn(Vec<SheetData>)>>;

/// Render an editable spreadsheet with sheet tabs and inline cell editing.
pub fn render_spreadsheet_editor(
    path: String,
    data: &DocData,
    set_open_files: WriteSignal<Vec<OpenFile>>,
) -> AnyView {
    let sheets = match data {
        DocData::Spreadsheet { sheets } => sheets.clone(),
        _ => return view! { <div class="spreadsheet-empty">"Not a spreadsheet"</div> }.into_any(),
    };
    if sheets.is_empty() {
        return view! { <div class="spreadsheet-empty">"Empty spreadsheet"</div> }.into_any();
    }

    let (active_tab, set_active_tab) = signal(0usize);
    let (sheets_sig, set_sheets) = signal(sheets);

    let file_path = path.clone();
    let sync_edits: SyncFn = SendWrapper::new(Rc::new(move |new_sheets: Vec<SheetData>| {
        set_sheets.set(new_sheets.clone());
        let p = file_path.clone();
        set_open_files.update(|fs| {
            if let Some(f) = fs.iter_mut().find(|f| f.path == p) {
                f.edited_doc_data = Some(DocData::Spreadsheet { sheets: new_sheets });
            }
        });
    }));

    let tab_list = move || {
        sheets_sig
            .get()
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.name.clone()))
            .collect::<Vec<_>>()
    };

    // Remove sheet handler
    let sync_rm_sheet = sync_edits.clone();
    let remove_sheet = SendWrapper::new(Rc::new(move |idx: usize| {
        let mut s = sheets_sig.get_untracked();
        if s.len() <= 1 {
            return;
        }
        s.remove(idx);
        let new_active = idx.min(s.len().saturating_sub(1));
        set_active_tab.set(new_active);
        sync_rm_sheet(s);
    }));

    view! {
        <div class="spreadsheet-viewer">
            <div class="spreadsheet-tabs">
                <For
                    each=tab_list
                    key=|(i, _)| *i
                    children={
                        let remove_sheet = remove_sheet.clone();
                        move |(i, name)| {
                            let remove_sheet = remove_sheet.clone();
                            view! {
                                <div class=move || if active_tab.get() == i { "spreadsheet-tab-group active" } else { "spreadsheet-tab-group" }>
                                    <button
                                        class="spreadsheet-tab-label"
                                        on:click=move |_| set_active_tab.set(i)
                                    >{name}</button>
                                    <button
                                        class="spreadsheet-tab-close"
                                        title="Remove sheet"
                                        on:click=move |_| remove_sheet(i)
                                    >"\u{00D7}"</button>
                                </div>
                            }
                        }
                    }
                />
                <button class="spreadsheet-tab spreadsheet-tab-add" title="Add sheet"
                    on:click={
                        let sync = sync_edits.clone();
                        move |_| {
                            let mut s = sheets_sig.get_untracked();
                            let n = s.len() + 1;
                            s.push(SheetData { name: format!("Sheet{n}"), rows: vec![vec!["".into()]] });
                            set_active_tab.set(s.len() - 1);
                            sync(s);
                        }
                    }
                >"+"</button>
            </div>
            <div class="spreadsheet-table-wrap">
                {move || {
                    let idx = active_tab.get();
                    let all = sheets_sig.get();
                    let sheet = match all.get(idx) {
                        Some(s) => s.clone(),
                        None => return view! { <div class="spreadsheet-empty">"No sheet"</div> }.into_any(),
                    };
                    let sync = sync_edits.clone();
                    render_editable_sheet(idx, &sheet, sheets_sig, sync)
                }}
            </div>
        </div>
    }
    .into_any()
}

/// Render a single sheet as an editable table with add/remove row/column.
fn render_editable_sheet(
    sheet_idx: usize,
    sheet: &SheetData,
    sheets_sig: ReadSignal<Vec<SheetData>>,
    sync: SyncFn,
) -> AnyView {
    let max_cols = sheet.rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let rows = sheet.rows.clone();

    let sync_add_row = sync.clone();
    let sync_add_col = sync.clone();
    let sync_rm_row = sync.clone();
    let sync_rm_col = sync.clone();

    let add_row = move |_| {
        let mut all = sheets_sig.get_untracked();
        if let Some(s) = all.get_mut(sheet_idx) {
            let cols = s.rows.iter().map(|r| r.len()).max().unwrap_or(1);
            s.rows.push(vec![String::new(); cols]);
        }
        sync_add_row(all);
    };
    let add_col = move |_| {
        let mut all = sheets_sig.get_untracked();
        if let Some(s) = all.get_mut(sheet_idx) {
            for row in &mut s.rows {
                row.push(String::new());
            }
        }
        sync_add_col(all);
    };
    let rm_last_row = move |_| {
        let mut all = sheets_sig.get_untracked();
        if let Some(s) = all.get_mut(sheet_idx) {
            if s.rows.len() > 1 {
                s.rows.pop();
            }
        }
        sync_rm_row(all);
    };
    let rm_last_col = move |_| {
        let mut all = sheets_sig.get_untracked();
        if let Some(s) = all.get_mut(sheet_idx) {
            for row in &mut s.rows {
                if row.len() > 1 {
                    row.pop();
                }
            }
        }
        sync_rm_col(all);
    };

    view! {
        <div class="spreadsheet-toolbar">
            <button class="spreadsheet-action-btn" on:click=add_row title="Add row">"+Row"</button>
            <button class="spreadsheet-action-btn" on:click=add_col title="Add column">"+Col"</button>
            <button class="spreadsheet-action-btn spreadsheet-action-danger" on:click=rm_last_row title="Remove last row">"-Row"</button>
            <button class="spreadsheet-action-btn spreadsheet-action-danger" on:click=rm_last_col title="Remove last column">"-Col"</button>
        </div>
        <table>
            <thead>
                <tr>
                    <th class="spreadsheet-row-num">"#"</th>
                    {(0..max_cols).map(|i| {
                        let letter = col_index_to_letter(i);
                        view! { <th>{letter}</th> }
                    }).collect::<Vec<_>>()}
                </tr>
            </thead>
            <tbody>
                {rows.iter().enumerate().map(|(r_idx, row)| {
                    let row_num = r_idx + 1;
                    let cells: Vec<_> = (0..max_cols).map(|c_idx| {
                        let value = row.get(c_idx).cloned().unwrap_or_default();
                        let sync = sync.clone();
                        let is_num = value.parse::<f64>().is_ok();
                        let class = if is_num { "spreadsheet-cell-num spreadsheet-cell-edit" } else { "spreadsheet-cell-edit" };
                        view! {
                            <td class=class
                                contenteditable="true"
                                on:blur=move |ev| {
                                    let el = ev.target().unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();
                                    let new_val = el.inner_text();
                                    let mut all = sheets_sig.get_untracked();
                                    if let Some(s) = all.get_mut(sheet_idx) {
                                        while s.rows.len() <= r_idx {
                                            s.rows.push(vec![String::new(); max_cols]);
                                        }
                                        while s.rows[r_idx].len() <= c_idx {
                                            s.rows[r_idx].push(String::new());
                                        }
                                        if s.rows[r_idx][c_idx] != new_val {
                                            s.rows[r_idx][c_idx] = new_val;
                                            sync(all);
                                        }
                                    }
                                }
                            >{value}</td>
                        }
                    }).collect();
                    view! {
                        <tr>
                            <td class="spreadsheet-row-num">{row_num}</td>
                            {cells}
                        </tr>
                    }
                }).collect::<Vec<_>>()}
            </tbody>
        </table>
    }
    .into_any()
}

/// Convert 0-based column index to Excel-style letter (A, B, ..., Z, AA, AB, ...).
fn col_index_to_letter(idx: usize) -> String {
    let mut result = String::new();
    let mut n = idx;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}
