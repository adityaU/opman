//! DOCX reading via quick-xml + zip.
//! Preserves underline, strikethrough, list items, nested inline formatting,
//! and tables for better round-trip fidelity.

use super::doc_readers::html_escape;
use crate::web::types::DocData;

pub fn read_docx(path: &std::path::Path) -> Result<DocData, String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|e| format!("Open docx: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Read docx zip: {e}"))?;

    let mut xml_content = String::new();
    {
        let mut doc_part = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("Find document.xml: {e}"))?;
        doc_part
            .read_to_string(&mut xml_content)
            .map_err(|e| format!("Read document.xml: {e}"))?;
    }

    let mut reader = Reader::from_str(&xml_content);
    let mut html_parts: Vec<String> = Vec::new();

    let mut run_pieces: Vec<String> = Vec::new();
    let mut bold = false;
    let mut italic = false;
    let mut underline = false;
    let mut strike = false;
    let mut in_paragraph = false;
    let mut heading_level: Option<u8> = None;
    let mut in_run_props = false;
    let mut in_list_item = false;

    let mut in_row = false;
    let mut row_cells: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"p" => {
                        in_paragraph = true;
                        run_pieces.clear();
                        heading_level = None;
                        in_list_item = false;
                        bold = false;
                        italic = false;
                        underline = false;
                        strike = false;
                    }
                    b"pStyle" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" || attr.key.as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                heading_level = parse_heading_level(&val);
                                if val.to_lowercase().contains("list") {
                                    in_list_item = true;
                                }
                            }
                        }
                    }
                    b"numId" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" || attr.key.as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                if val != "0" {
                                    in_list_item = true;
                                }
                            }
                        }
                    }
                    b"rPr" => {
                        in_run_props = true;
                        bold = false;
                        italic = false;
                        underline = false;
                        strike = false;
                    }
                    b"b" if in_run_props => bold = true,
                    b"i" if in_run_props => italic = true,
                    b"u" if in_run_props => underline = true,
                    b"strike" if in_run_props => strike = true,
                    b"tbl" => table_rows.clear(),
                    b"tr" => {
                        in_row = true;
                        row_cells.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if !in_paragraph {
                    continue;
                }
                let Ok(text) = e.unescape() else {
                    continue;
                };
                let escaped = html_escape(&text);
                let mut piece = escaped.clone();
                if bold {
                    piece = format!("<b>{piece}</b>");
                }
                if italic {
                    piece = format!("<i>{piece}</i>");
                }
                if underline {
                    piece = format!("<u>{piece}</u>");
                }
                if strike {
                    piece = format!("<s>{piece}</s>");
                }
                run_pieces.push(piece);
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"rPr" => in_run_props = false,
                    b"p" => {
                        if in_paragraph {
                            let combined: String = run_pieces.join("");
                            let trimmed = combined.trim();
                            if !trimmed.is_empty() {
                                if in_row {
                                    row_cells.push(trimmed.to_string());
                                } else if in_list_item {
                                    html_parts.push(format!("<li>{trimmed}</li>"));
                                } else {
                                    let tag = heading_tag(heading_level);
                                    html_parts.push(format!("<{tag}>{trimmed}</{tag}>"));
                                }
                            }
                        }
                        in_paragraph = false;
                        run_pieces.clear();
                    }
                    b"tr" => {
                        if in_row && !row_cells.is_empty() {
                            table_rows.push(row_cells.clone());
                        }
                        in_row = false;
                        row_cells.clear();
                    }
                    b"tbl" => {
                        if !table_rows.is_empty() {
                            html_parts.push(format_table_html(&table_rows));
                        }
                        table_rows.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
    }

    let html = wrap_list_items(&html_parts.join("\n"));
    Ok(DocData::Document { html })
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Wrap consecutive `<li>...</li>` runs in `<ul>...</ul>`.
fn wrap_list_items(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_list = false;
    for line in html.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<li>") {
            if !in_list {
                out.push_str("<ul>\n");
                in_list = true;
            }
            out.push_str(trimmed);
            out.push('\n');
        } else {
            if in_list {
                out.push_str("</ul>\n");
                in_list = false;
            }
            out.push_str(line);
            out.push('\n');
        }
    }
    if in_list {
        out.push_str("</ul>\n");
    }
    out
}

fn heading_tag(level: Option<u8>) -> &'static str {
    match level {
        Some(1) => "h1",
        Some(2) => "h2",
        Some(3) => "h3",
        Some(4) => "h4",
        _ => "p",
    }
}

fn parse_heading_level(style_val: &str) -> Option<u8> {
    let lower = style_val.to_lowercase();
    if !lower.starts_with("heading") && !lower.starts_with("titre") {
        return None;
    }
    lower
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u8>()
        .ok()
        .filter(|&l| (1..=6).contains(&l))
}

fn format_table_html(rows: &[Vec<String>]) -> String {
    let mut html = String::from("<table>");
    for (i, row) in rows.iter().enumerate() {
        html.push_str("<tr>");
        let tag = if i == 0 { "th" } else { "td" };
        for cell in row {
            html.push_str(&format!("<{tag}>{cell}</{tag}>"));
        }
        html.push_str("</tr>");
    }
    html.push_str("</table>");
    html
}
