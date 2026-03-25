//! Document format writing: spreadsheets (xlsx/tsv) and documents (docx).
//! HTML sanitization/parsing lives in sibling module `doc_writers_html`.

use super::doc_writers_html::{html_unescape, parse_blocks, sanitize_html};
use crate::web::types::DocData;

// ── Spreadsheet writing via rust_xlsxwriter ─────────────────────────

pub fn write_xlsx(path: &std::path::Path, data: &DocData) -> Result<(), String> {
    use rust_xlsxwriter::Workbook;

    let sheets = match data {
        DocData::Spreadsheet { sheets } => sheets,
        _ => return Err("Expected spreadsheet data".into()),
    };

    let mut wb = Workbook::new();
    for sheet in sheets {
        let ws = wb
            .add_worksheet()
            .set_name(&sheet.name)
            .map_err(|e| e.to_string())?;
        for (r, row) in sheet.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if let Ok(num) = cell.parse::<f64>() {
                    let _ = ws.write_number(r as u32, c as u16, num);
                } else {
                    let _ = ws.write_string(r as u32, c as u16, cell);
                }
            }
        }
    }
    wb.save(path).map_err(|e| format!("Write xlsx: {e}"))
}

// ── TSV writing ─────────────────────────────────────────────────────

pub fn write_tsv(path: &std::path::Path, data: &DocData) -> Result<(), String> {
    let sheets = match data {
        DocData::Spreadsheet { sheets } => sheets,
        _ => return Err("Expected spreadsheet data".into()),
    };
    let rows = sheets.first().map(|s| &s.rows).ok_or("No sheets")?;
    let mut out = String::new();
    for row in rows {
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|e| format!("Write TSV: {e}"))
}

// ── DOCX writing via docx crate ─────────────────────────────────────

pub fn write_docx(path: &std::path::Path, data: &DocData) -> Result<(), String> {
    use docx::document::{BodyContent, Paragraph, Run, RunContent, Text};
    use docx::formatting::{CharacterProperty, ParagraphProperty, UnderlineStyle};
    use docx::Docx;

    let html = match data {
        DocData::Document { html } => html,
        _ => return Err("Expected document data".into()),
    };

    let clean = sanitize_html(html);
    let mut docx = Docx::default();

    for block in parse_blocks(&clean) {
        if block.text.trim().is_empty() && block.runs.is_empty() {
            continue;
        }
        let mut para = Paragraph::default();
        if let Some(level) = block.heading_level {
            let style = match level {
                1 => "Heading1",
                2 => "Heading2",
                3 => "Heading3",
                4 => "Heading4",
                _ => "Heading5",
            };
            let pp = ParagraphProperty::default().style_id(style);
            para = para.property(pp);
        }
        if block.runs.is_empty() {
            let text = html_unescape(&block.text);
            para = para.push(Run::default().push(RunContent::Text(Text::from(text))));
        } else {
            for ri in &block.runs {
                let text = html_unescape(&ri.text);
                let mut run = Run::default();
                if ri.bold || ri.italic || ri.underline || ri.strike {
                    let mut cp = CharacterProperty::default();
                    if ri.bold {
                        cp = cp.bold(true);
                    }
                    if ri.italic {
                        cp = cp.italics(true);
                    }
                    if ri.underline {
                        cp = cp.underline(UnderlineStyle::Single);
                    }
                    if ri.strike {
                        cp = cp.strike(true);
                    }
                    run = run.property(cp);
                }
                run = run.push(RunContent::Text(Text::from(text)));
                para = para.push(run);
            }
        }
        docx.document.push(BodyContent::Paragraph(para));
    }

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let cursor = docx
        .write(cursor)
        .map_err(|e| format!("Write docx: {e:?}"))?;
    std::fs::write(path, cursor.into_inner()).map_err(|e| format!("Save docx: {e}"))
}
