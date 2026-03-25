//! Document format reading: spreadsheets (xlsx/xls/ods/xlsb/tsv) and documents (docx).
//! DOCX-specific reading lives in sibling module `doc_readers_docx`.

use crate::web::types::{DocData, SheetData};

pub use super::doc_readers_docx::read_docx;

// ── Spreadsheet reading via calamine ────────────────────────────────

pub fn read_spreadsheet(path: &std::path::Path) -> Result<DocData, String> {
    use calamine::{open_workbook_auto, Data, Reader};

    let mut wb = open_workbook_auto(path).map_err(|e| format!("Open spreadsheet: {e}"))?;
    let sheet_names: Vec<String> = wb.sheet_names().to_vec();
    let mut sheets = Vec::with_capacity(sheet_names.len());

    for name in &sheet_names {
        let range = wb
            .worksheet_range(name)
            .map_err(|e| format!("Read sheet '{name}': {e}"))?;
        let mut rows = Vec::new();
        for row in range.rows() {
            let cells: Vec<String> = row
                .iter()
                .map(|c| match c {
                    Data::Empty => String::new(),
                    Data::String(s) => s.clone(),
                    Data::Float(f) => format_float(*f),
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => b.to_string(),
                    Data::Error(e) => format!("#ERR({e:?})"),
                    _ => c.to_string(),
                })
                .collect();
            let trimmed = trim_trailing_empty(cells);
            if !trimmed.is_empty() {
                rows.push(trimmed);
            }
        }
        sheets.push(SheetData {
            name: name.clone(),
            rows,
        });
    }

    Ok(DocData::Spreadsheet { sheets })
}

// ── TSV reading ─────────────────────────────────────────────────────

pub fn read_tsv(path: &std::path::Path) -> Result<DocData, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Read TSV: {e}"))?;
    let mut rows = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        rows.push(line.split('\t').map(|s| s.to_string()).collect());
    }
    Ok(DocData::Spreadsheet {
        sheets: vec![SheetData {
            name: "Sheet1".into(),
            rows,
        }],
    })
}

// ── Shared helpers ──────────────────────────────────────────────────

fn format_float(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e15 {
        format!("{:.0}", f)
    } else {
        f.to_string()
    }
}

fn trim_trailing_empty(mut cells: Vec<String>) -> Vec<String> {
    while cells.last().map(|c| c.is_empty()).unwrap_or(false) {
        cells.pop();
    }
    cells
}

pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
