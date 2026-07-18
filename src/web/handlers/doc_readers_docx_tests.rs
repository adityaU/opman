//! Generated tests for `doc_readers_docx.rs` (docx parsing + helpers).
use super::*;
use crate::web::types::DocData;
use std::io::Write;

/// Build a minimal `.docx` (a zip containing `word/document.xml`) at `path`.
fn build_docx(path: &std::path::Path, body_inner: &str) {
    let doc = format!(
        "<?xml version=\"1.0\"?><w:document \
         xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:body>{body_inner}</w:body></w:document>"
    );
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(doc.as_bytes()).unwrap();
    zip.finish().unwrap();
}

fn read_html(body_inner: &str) -> String {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("doc.docx");
    build_docx(&path, body_inner);
    match read_docx(&path).unwrap() {
        DocData::Document { html } => html,
        _ => panic!("expected document"),
    }
}

#[test]
fn read_docx_headings() {
    let html = read_html(
        "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>\
         <w:p><w:pPr><w:pStyle w:val=\"Heading2\"/></w:pPr><w:r><w:t>Sub</w:t></w:r></w:p>\
         <w:p><w:pPr><w:pStyle w:val=\"Heading3\"/></w:pPr><w:r><w:t>H3</w:t></w:r></w:p>\
         <w:p><w:pPr><w:pStyle w:val=\"Heading4\"/></w:pPr><w:r><w:t>H4</w:t></w:r></w:p>",
    );
    assert!(html.contains("<h1>Title</h1>"));
    assert!(html.contains("<h2>Sub</h2>"));
    assert!(html.contains("<h3>H3</h3>"));
    assert!(html.contains("<h4>H4</h4>"));
}

#[test]
fn read_docx_plain_paragraph() {
    let html = read_html("<w:p><w:r><w:t>Hello world</w:t></w:r></w:p>");
    assert!(html.contains("<p>Hello world</p>"), "got {html}");
}

#[test]
fn read_docx_inline_formatting() {
    let html = read_html(
        "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r></w:p>\
         <w:p><w:r><w:rPr><w:i/></w:rPr><w:t>ital</w:t></w:r></w:p>\
         <w:p><w:r><w:rPr><w:u/></w:rPr><w:t>und</w:t></w:r></w:p>\
         <w:p><w:r><w:rPr><w:strike/></w:rPr><w:t>str</w:t></w:r></w:p>",
    );
    assert!(html.contains("<b>bold</b>"), "got {html}");
    assert!(html.contains("<i>ital</i>"));
    assert!(html.contains("<u>und</u>"));
    assert!(html.contains("<s>str</s>"));
}

#[test]
fn read_docx_list_items_via_style_and_numid() {
    let html = read_html(
        "<w:p><w:pPr><w:pStyle w:val=\"ListParagraph\"/></w:pPr><w:r><w:t>one</w:t></w:r></w:p>\
         <w:p><w:pPr><w:numPr><w:numId w:val=\"2\"/></w:numPr></w:pPr><w:r><w:t>two</w:t></w:r></w:p>",
    );
    assert!(html.contains("<ul>"), "got {html}");
    assert!(html.contains("<li>one</li>"));
    assert!(html.contains("<li>two</li>"));
    assert!(html.contains("</ul>"));
}

#[test]
fn read_docx_numid_zero_not_list() {
    let html = read_html(
        "<w:p><w:pPr><w:numPr><w:numId w:val=\"0\"/></w:numPr></w:pPr><w:r><w:t>plain</w:t></w:r></w:p>",
    );
    assert!(!html.contains("<li>"), "got {html}");
    assert!(html.contains("<p>plain</p>"));
}

#[test]
fn read_docx_table() {
    let html = read_html(
        "<w:tbl>\
         <w:tr><w:tc><w:p><w:r><w:t>H1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>H2</w:t></w:r></w:p></w:tc></w:tr>\
         <w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr>\
         </w:tbl>",
    );
    assert!(html.contains("<table>"), "got {html}");
    assert!(html.contains("<th>H1</th>"));
    assert!(html.contains("<th>H2</th>"));
    assert!(html.contains("<td>a</td>"));
    assert!(html.contains("<td>b</td>"));
}

#[test]
fn read_docx_empty_paragraph_skipped_and_text_outside_para_ignored() {
    // Leading text is outside any <w:p> and must be ignored; empty para produces nothing.
    let html = read_html("orphan text<w:p></w:p><w:p><w:r><w:t>real</w:t></w:r></w:p>");
    assert!(!html.contains("orphan"), "got {html}");
    assert!(html.contains("<p>real</p>"));
}

#[test]
fn read_docx_entity_unescaped_then_reescaped() {
    let html = read_html("<w:p><w:r><w:t>a &amp; b</w:t></w:r></w:p>");
    assert!(html.contains("a &amp; b"), "got {html}");
}

#[test]
fn read_docx_open_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nope.docx");
    let err = read_docx(&path).unwrap_err();
    assert!(err.starts_with("Open docx:"), "got {err}");
}

#[test]
fn read_docx_not_a_zip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("bad.docx");
    std::fs::write(&path, b"definitely not a zip archive").unwrap();
    let err = read_docx(&path).unwrap_err();
    assert!(err.starts_with("Read docx zip:"), "got {err}");
}

#[test]
fn read_docx_missing_document_xml() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("empty.docx");
    // A valid zip but with no word/document.xml part.
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    zip.start_file("other.txt", opts).unwrap();
    zip.write_all(b"hi").unwrap();
    zip.finish().unwrap();
    let err = read_docx(&path).unwrap_err();
    assert!(err.starts_with("Find document.xml:"), "got {err}");
}

#[test]
fn read_docx_malformed_xml() {
    // An unterminated start tag triggers a quick-xml parse error at EOF.
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("mal.docx");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(b"<w:body><w:p").unwrap();
    zip.finish().unwrap();
    let err = read_docx(&path).unwrap_err();
    assert!(err.starts_with("XML parse error:"), "got {err}");
}

// ── Helper-level tests ──────────────────────────────────────────────

#[test]
fn wrap_list_items_wraps_consecutive_li() {
    let input = "<p>before</p>\n<li>a</li>\n<li>b</li>\n<p>after</p>";
    let out = wrap_list_items(input);
    assert!(out.contains("<ul>\n<li>a</li>\n<li>b</li>\n</ul>"), "got {out}");
    assert!(out.contains("<p>before</p>"));
    assert!(out.contains("<p>after</p>"));
}

#[test]
fn wrap_list_items_trailing_list_closes_ul() {
    let out = wrap_list_items("<li>only</li>");
    assert!(out.contains("<ul>"));
    assert!(out.trim_end().ends_with("</ul>"), "got {out}");
}

#[test]
fn wrap_list_items_no_lists() {
    let out = wrap_list_items("<p>x</p>\n<p>y</p>");
    assert!(!out.contains("<ul>"));
}

#[test]
fn heading_tag_mapping() {
    assert_eq!(heading_tag(Some(1)), "h1");
    assert_eq!(heading_tag(Some(2)), "h2");
    assert_eq!(heading_tag(Some(3)), "h3");
    assert_eq!(heading_tag(Some(4)), "h4");
    assert_eq!(heading_tag(Some(5)), "p");
    assert_eq!(heading_tag(None), "p");
}

#[test]
fn parse_heading_level_variants() {
    assert_eq!(parse_heading_level("Heading1"), Some(1));
    assert_eq!(parse_heading_level("heading 3"), Some(3));
    assert_eq!(parse_heading_level("Titre2"), Some(2));
    // Out of the 1..=6 range -> filtered out.
    assert_eq!(parse_heading_level("Heading9"), None);
    // Not a heading style at all.
    assert_eq!(parse_heading_level("Normal"), None);
    // Heading prefix but no digit -> parse fails.
    assert_eq!(parse_heading_level("heading"), None);
}

#[test]
fn format_table_html_headers_and_body() {
    let rows = vec![
        vec!["A".to_string(), "B".to_string()],
        vec!["1".to_string(), "2".to_string()],
    ];
    let out = format_table_html(&rows);
    assert_eq!(
        out,
        "<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>"
    );
}
