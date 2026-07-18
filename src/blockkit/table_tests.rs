use super::*;

use crate::blockkit::TableAlign;

// ── parse_row ───────────────────────────────────────────────────────────

#[test]
fn parse_row_strips_outer_pipes_and_trims() {
    let cells = parse_row("| a | b | c |");
    assert_eq!(cells, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
}

#[test]
fn parse_row_without_outer_pipes() {
    let cells = parse_row("a|b");
    assert_eq!(cells, vec!["a".to_string(), "b".to_string()]);
}

// ── parse_alignments ────────────────────────────────────────────────────

#[test]
fn parse_alignments_all_variants() {
    let al = parse_alignments("| :--- | :---: | ---: | --- |");
    assert_eq!(al, vec![
        TableAlign::Left,
        TableAlign::Center,
        TableAlign::Right,
        TableAlign::Left,
    ]);
}

// ── is_separator ────────────────────────────────────────────────────────

#[test]
fn is_separator_true() {
    assert!(is_separator("| --- | :---: |"));
    assert!(is_separator("|---|"));
}

#[test]
fn is_separator_false_no_pipe() {
    assert!(!is_separator("--- ---"));
}

#[test]
fn is_separator_false_has_text() {
    assert!(!is_separator("| abc | def |"));
}

#[test]
fn is_separator_false_no_dash() {
    // pipes and colons only, no dash → not a separator.
    assert!(!is_separator("| : | : |"));
}

// ── is_table_line ───────────────────────────────────────────────────────

#[test]
fn is_table_line_variants() {
    assert!(is_table_line("| a | b |"));
    assert!(!is_table_line(""));
    assert!(!is_table_line("no pipes here"));
    assert!(!is_table_line("   "));
}

// ── parse_table ─────────────────────────────────────────────────────────

#[test]
fn parse_table_basic() {
    let lines = vec![
        "| Name | Age |",
        "| --- | ---: |",
        "| Alice | 30 |",
        "| Bob | 25 |",
    ];
    let (headers, aligns, rows, consumed) = parse_table(&lines, 0);
    assert_eq!(headers, vec!["Name".to_string(), "Age".to_string()]);
    assert_eq!(aligns, vec![TableAlign::Left, TableAlign::Right]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["Alice".to_string(), "30".to_string()]);
    assert_eq!(consumed, 4);
}

#[test]
fn parse_table_stops_at_non_table_line() {
    let lines = vec![
        "| A | B |",
        "| --- | --- |",
        "| 1 | 2 |",
        "plain text after",
    ];
    let (_h, _a, rows, consumed) = parse_table(&lines, 0);
    assert_eq!(rows.len(), 1);
    assert_eq!(consumed, 3);
}

#[test]
fn parse_table_no_data_rows() {
    let lines = vec!["| A | B |", "| --- | --- |"];
    let (headers, aligns, rows, consumed) = parse_table(&lines, 0);
    assert_eq!(headers.len(), 2);
    assert_eq!(aligns.len(), 2);
    assert!(rows.is_empty());
    assert_eq!(consumed, 2);
}

// ── split_around_tables extra edge cases ────────────────────────────────

#[test]
fn split_leading_blank_before_table_no_empty_text_segment() {
    let md = "   \n\n| A | B |\n| --- | --- |\n| 1 | 2 |";
    let segs = split_around_tables(md);
    assert_eq!(segs.len(), 1);
    assert!(matches!(&segs[0], MdTextSegment::Table(_)));
}

#[test]
fn split_trailing_blank_after_table_no_empty_text_segment() {
    let md = "| A | B |\n| --- | --- |\n| 1 | 2 |\n   ";
    let segs = split_around_tables(md);
    // The table absorbs contiguous table lines; trailing whitespace-only
    // text is dropped.
    assert!(matches!(&segs[0], MdTextSegment::Table(_)));
    assert!(segs.iter().all(|s| match s {
        MdTextSegment::Text(t) => !t.trim().is_empty(),
        MdTextSegment::Table(_) => true,
    }));
}

#[test]
fn split_empty_input() {
    let segs = split_around_tables("");
    assert!(segs.is_empty());
}

#[test]
fn md_text_segment_debug() {
    let t = MdTextSegment::Text("x".into());
    let tbl = MdTextSegment::Table("y".into());
    assert!(format!("{:?}", t).contains("Text"));
    assert!(format!("{:?}", tbl).contains("Table"));
}
