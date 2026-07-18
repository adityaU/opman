//! Generated tests for `doc_writers.rs` (xlsx/tsv/docx writing + helpers).
use super::*;
use crate::web::handlers::doc_readers::{read_docx, read_spreadsheet};
use crate::web::types::{DocData, SheetData};

fn sheet(name: &str, rows: Vec<Vec<&str>>) -> SheetData {
    SheetData {
        name: name.to_string(),
        rows: rows
            .into_iter()
            .map(|r| r.into_iter().map(|s| s.to_string()).collect())
            .collect(),
    }
}

#[test]
fn write_xlsx_round_trips() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.xlsx");
    let data = DocData::Spreadsheet {
        sheets: vec![sheet("S1", vec![vec!["Name", "Score"], vec!["Al", "42"]])],
    };
    write_xlsx(&path, &data).unwrap();
    assert!(path.exists());

    let DocData::Spreadsheet { sheets } = read_spreadsheet(&path).unwrap() else {
        panic!("expected spreadsheet");
    };
    assert_eq!(sheets[0].name, "S1");
    assert_eq!(sheets[0].rows[0], vec!["Name", "Score"]);
    // "42" was numeric -> written as a number -> read back as "42".
    assert_eq!(sheets[0].rows[1], vec!["Al", "42"]);
}

#[test]
fn write_xlsx_wrong_variant_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.xlsx");
    let data = DocData::Document { html: "<p>x</p>".into() };
    let err = write_xlsx(&path, &data).unwrap_err();
    assert_eq!(err, "Expected spreadsheet data");
}

#[test]
fn write_xlsx_bad_path_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    // Parent directory does not exist.
    let path = dir.path().join("missing_dir").join("out.xlsx");
    let data = DocData::Spreadsheet {
        sheets: vec![sheet("S", vec![vec!["a"]])],
    };
    let err = write_xlsx(&path, &data).unwrap_err();
    assert!(err.starts_with("Write xlsx:"), "got {err}");
}

#[test]
fn write_tsv_writes_rows() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.tsv");
    let data = DocData::Spreadsheet {
        sheets: vec![sheet("S", vec![vec!["a", "b"], vec!["1", "2"]])],
    };
    write_tsv(&path, &data).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "a\tb\n1\t2\n");
}

#[test]
fn write_tsv_wrong_variant_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.tsv");
    let data = DocData::Document { html: "x".into() };
    assert_eq!(write_tsv(&path, &data).unwrap_err(), "Expected spreadsheet data");
}

#[test]
fn write_tsv_no_sheets_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.tsv");
    let data = DocData::Spreadsheet { sheets: vec![] };
    assert_eq!(write_tsv(&path, &data).unwrap_err(), "No sheets");
}

#[test]
fn write_tsv_bad_path_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nope").join("out.tsv");
    let data = DocData::Spreadsheet {
        sheets: vec![sheet("S", vec![vec!["a"]])],
    };
    let err = write_tsv(&path, &data).unwrap_err();
    assert!(err.starts_with("Write TSV:"), "got {err}");
}

#[test]
fn write_docx_round_trips_headings_and_runs() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.docx");
    let html = "<h1>Title</h1><p>plain</p><p><b>bold</b> and <i>ital</i> and <u>u</u> and <s>st</s></p>";
    let data = DocData::Document { html: html.into() };
    write_docx(&path, &data).unwrap();
    assert!(path.exists());

    let DocData::Document { html: back } = read_docx(&path).unwrap() else {
        panic!("expected document");
    };
    assert!(back.contains("<h1>Title</h1>"), "got {back}");
    assert!(back.contains("plain"));
    assert!(back.contains("<b>bold</b>"));
    assert!(back.contains("<i>ital</i>"));
    assert!(back.contains("<u>u</u>"));
    assert!(back.contains("<s>st</s>"));
}

#[test]
fn write_docx_covers_all_heading_levels() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("h.docx");
    let html = "<h1>a</h1><h2>b</h2><h3>c</h3><h4>d</h4><h5>e</h5><h6>f</h6>";
    let data = DocData::Document { html: html.into() };
    write_docx(&path, &data).unwrap();
    // Round-trip: h5/h6 map to Heading5 style -> read back as <p> (reader caps at 4),
    // just assert the file is produced and non-trivial.
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
}

#[test]
fn write_docx_wrong_variant_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("out.docx");
    let data = DocData::Spreadsheet { sheets: vec![] };
    assert_eq!(write_docx(&path, &data).unwrap_err(), "Expected document data");
}

#[test]
fn write_docx_bad_path_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nope").join("out.docx");
    let data = DocData::Document { html: "<p>x</p>".into() };
    let err = write_docx(&path, &data).unwrap_err();
    assert!(err.starts_with("Save docx:"), "got {err}");
}

#[test]
fn write_docx_empty_html_produces_file() {
    // Empty/whitespace-only blocks are skipped in build_body_xml.
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("empty.docx");
    let data = DocData::Document { html: "<p>   </p>".into() };
    write_docx(&path, &data).unwrap();
    assert!(path.exists());
}

#[test]
fn ze_formats_error() {
    assert_eq!(ze("boom"), "Write docx zip: boom");
}

#[test]
fn xml_escape_all_specials() {
    assert_eq!(
        xml_escape("a & b < c > d \" e ' f"),
        "a &amp; b &lt; c &gt; d &quot; e &apos; f"
    );
    assert_eq!(xml_escape("plain"), "plain");
}

#[test]
fn build_body_xml_direct() {
    let out = build_body_xml("<h2>Head</h2><p><b>b</b></p>");
    assert!(out.contains("<w:pStyle w:val=\"Heading2\"/>"), "got {out}");
    assert!(out.contains("<w:b/>"));
    assert!(out.contains("<w:body>"));
    assert!(out.starts_with("<?xml"));
}

#[test]
fn build_body_xml_all_heading_styles() {
    let out = build_body_xml("<h1>1</h1><h3>3</h3><h4>4</h4><h5>5</h5>");
    assert!(out.contains("Heading1"));
    assert!(out.contains("Heading3"));
    assert!(out.contains("Heading4"));
    assert!(out.contains("Heading5"));
}
