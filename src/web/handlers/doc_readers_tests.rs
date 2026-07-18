//! Generated tests for `doc_readers.rs` (spreadsheet + TSV reading, helpers).
use super::*;
use crate::web::types::DocData;

/// Build an xlsx workbook at `path` for reading-round-trip tests.
fn build_xlsx(path: &std::path::Path) {
    use rust_xlsxwriter::Workbook;
    let mut wb = Workbook::new();
    {
        let ws = wb.add_worksheet().set_name("First").unwrap();
        ws.write_string(0, 0, "Name").unwrap();
        ws.write_string(0, 1, "Age").unwrap();
        // Row 1 intentionally left blank -> should be skipped by reader.
        ws.write_string(2, 0, "Alice").unwrap();
        ws.write_number(2, 1, 30.0).unwrap();
        ws.write_string(3, 0, "flag").unwrap();
        ws.write_boolean(3, 1, true).unwrap();
    }
    {
        let ws = wb.add_worksheet().set_name("Second").unwrap();
        ws.write_number(0, 0, 3.5).unwrap();
    }
    wb.save(path).unwrap();
}

#[test]
fn read_spreadsheet_parses_cells_and_sheets() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("book.xlsx");
    build_xlsx(&path);

    let data = read_spreadsheet(&path).unwrap();
    let DocData::Spreadsheet { sheets } = data else {
        panic!("expected spreadsheet");
    };
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[0].name, "First");
    // Blank row 1 was dropped, so we have 3 non-empty rows.
    assert_eq!(sheets[0].rows.len(), 3);
    assert_eq!(sheets[0].rows[0], vec!["Name", "Age"]);
    // 30.0 integer-valued float formats without decimals.
    assert_eq!(sheets[0].rows[1], vec!["Alice", "30"]);
    // Boolean cell.
    assert_eq!(sheets[0].rows[2], vec!["flag", "true"]);
    assert_eq!(sheets[1].name, "Second");
    assert_eq!(sheets[1].rows[0], vec!["3.5"]);
}

#[test]
fn read_spreadsheet_open_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("does_not_exist.xlsx");
    let err = read_spreadsheet(&path).unwrap_err();
    assert!(err.starts_with("Open spreadsheet:"), "got {err}");
}

#[test]
fn read_spreadsheet_bad_content_errors() {
    // A file with an .xlsx name but garbage content should fail to open.
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("garbage.xlsx");
    std::fs::write(&path, b"not a real spreadsheet").unwrap();
    assert!(read_spreadsheet(&path).is_err());
}

#[test]
fn read_tsv_parses_rows_and_skips_empty_lines() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("data.tsv");
    std::fs::write(&path, "a\tb\tc\n\n1\t2\t3\n").unwrap();

    let data = read_tsv(&path).unwrap();
    let DocData::Spreadsheet { sheets } = data else {
        panic!("expected spreadsheet");
    };
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].name, "Sheet1");
    assert_eq!(sheets[0].rows.len(), 2);
    assert_eq!(sheets[0].rows[0], vec!["a", "b", "c"]);
    assert_eq!(sheets[0].rows[1], vec!["1", "2", "3"]);
}

#[test]
fn read_tsv_missing_file_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("missing.tsv");
    let err = read_tsv(&path).unwrap_err();
    assert!(err.starts_with("Read TSV:"), "got {err}");
}

#[test]
fn format_float_variants() {
    assert_eq!(format_float(30.0), "30");
    assert_eq!(format_float(-5.0), "-5");
    assert_eq!(format_float(0.0), "0");
    assert_eq!(format_float(3.14), "3.14");
    // Very large magnitude falls back to Rust's default formatting.
    assert_eq!(format_float(1e20), 1e20.to_string());
}

#[test]
fn trim_trailing_empty_variants() {
    assert_eq!(
        trim_trailing_empty(vec!["a".into(), "b".into(), "".into(), "".into()]),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(
        trim_trailing_empty(vec!["a".into(), "b".into()]),
        vec!["a".to_string(), "b".to_string()]
    );
    let empty: Vec<String> = trim_trailing_empty(vec!["".into(), "".into()]);
    assert!(empty.is_empty());
    let none: Vec<String> = trim_trailing_empty(Vec::new());
    assert!(none.is_empty());
}

#[test]
fn html_escape_escapes_special_chars() {
    assert_eq!(html_escape("a & b < c > d"), "a &amp; b &lt; c &gt; d");
    assert_eq!(html_escape("plain"), "plain");
    assert_eq!(html_escape(""), "");
}
