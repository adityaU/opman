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

// ── DOCX writing via manual XML + ZIP (replaces docx crate) ─────────

pub fn write_docx(path: &std::path::Path, data: &DocData) -> Result<(), String> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let html = match data {
        DocData::Document { html } => html,
        _ => return Err("Expected document data".into()),
    };

    let clean = sanitize_html(html);
    let body_xml = build_body_xml(&clean);

    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut zip = ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", opts).map_err(ze)?;
    zip.write_all(CONTENT_TYPES_XML.as_bytes()).map_err(ze)?;

    zip.start_file("_rels/.rels", opts).map_err(ze)?;
    zip.write_all(RELS_XML.as_bytes()).map_err(ze)?;

    zip.start_file("word/_rels/document.xml.rels", opts)
        .map_err(ze)?;
    zip.write_all(DOC_RELS_XML.as_bytes()).map_err(ze)?;

    zip.start_file("word/styles.xml", opts).map_err(ze)?;
    zip.write_all(STYLES_XML.as_bytes()).map_err(ze)?;

    zip.start_file("word/document.xml", opts).map_err(ze)?;
    zip.write_all(body_xml.as_bytes()).map_err(ze)?;

    let cursor = zip.finish().map_err(ze)?;
    std::fs::write(path, cursor.into_inner()).map_err(|e| format!("Save docx: {e}"))
}

fn ze<E: std::fmt::Display>(e: E) -> String {
    format!("Write docx zip: {e}")
}

fn build_body_xml(sanitized_html: &str) -> String {
    let mut body = String::new();
    for block in parse_blocks(sanitized_html) {
        if block.text.trim().is_empty() && block.runs.is_empty() {
            continue;
        }
        body.push_str("<w:p>");
        if let Some(level) = block.heading_level {
            let style = match level {
                1 => "Heading1",
                2 => "Heading2",
                3 => "Heading3",
                4 => "Heading4",
                _ => "Heading5",
            };
            body.push_str(&format!("<w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>"));
        }
        if block.runs.is_empty() {
            let text = xml_escape(&html_unescape(&block.text));
            body.push_str(&format!(
                "<w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r>"
            ));
        } else {
            for ri in &block.runs {
                let text = xml_escape(&html_unescape(&ri.text));
                body.push_str("<w:r>");
                if ri.bold || ri.italic || ri.underline || ri.strike {
                    body.push_str("<w:rPr>");
                    if ri.bold {
                        body.push_str("<w:b/>");
                    }
                    if ri.italic {
                        body.push_str("<w:i/>");
                    }
                    if ri.underline {
                        body.push_str("<w:u w:val=\"single\"/>");
                    }
                    if ri.strike {
                        body.push_str("<w:strike/>");
                    }
                    body.push_str("</w:rPr>");
                }
                body.push_str(&format!("<w:t xml:space=\"preserve\">{text}</w:t></w:r>"));
            }
        }
        body.push_str("</w:p>");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:document xmlns:wpc=\"http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas\" \
         xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
         xmlns:o=\"urn:schemas-microsoft-com:office:office\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" \
         xmlns:v=\"urn:schemas-microsoft-com:vml\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:w10=\"urn:schemas-microsoft-com:office:word\" \
         xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
         xmlns:wne=\"http://schemas.microsoft.com/office/word/2006/wordml\">\
         <w:body>{body}</w:body></w:document>"
    )
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ── Static OOXML boilerplate ────────────────────────────────────────

const CONTENT_TYPES_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" \
ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" \
ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
</Types>";

const RELS_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
Target=\"word/document.xml\"/>\
</Relationships>";

const DOC_RELS_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" \
Target=\"styles.xml\"/>\
</Relationships>";

const STYLES_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/>\
<w:pPr><w:outlineLvl w:val=\"0\"/></w:pPr>\
<w:rPr><w:b/><w:sz w:val=\"48\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading2\"><w:name w:val=\"heading 2\"/>\
<w:pPr><w:outlineLvl w:val=\"1\"/></w:pPr>\
<w:rPr><w:b/><w:sz w:val=\"36\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading3\"><w:name w:val=\"heading 3\"/>\
<w:pPr><w:outlineLvl w:val=\"2\"/></w:pPr>\
<w:rPr><w:b/><w:sz w:val=\"28\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading4\"><w:name w:val=\"heading 4\"/>\
<w:pPr><w:outlineLvl w:val=\"3\"/></w:pPr>\
<w:rPr><w:b/><w:i/><w:sz w:val=\"24\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading5\"><w:name w:val=\"heading 5\"/>\
<w:pPr><w:outlineLvl w:val=\"4\"/></w:pPr>\
<w:rPr><w:sz w:val=\"22\"/></w:rPr></w:style>\
</w:styles>";
