//! Conversion tests. Every case here is one where being wrong produces a
//! confident, plausible, incorrect answer rather than a visible failure.

use super::*;

#[test]
fn uri_round_trips_including_spaces() {
    let path = Path::new("/home/u/my project/src/main.rs");
    let uri = path_to_uri(path);
    assert!(uri.starts_with("file:///home/u/my%20project/"));
    assert_eq!(uri_to_path(&uri).unwrap(), path);
}

#[test]
fn uri_round_trips_non_ascii() {
    let path = Path::new("/tmp/café/naïve.rs");
    assert_eq!(uri_to_path(&path_to_uri(path)).unwrap(), path);
}

#[test]
fn non_file_uris_are_rejected() {
    assert!(uri_to_path("untitled:Untitled-1").is_none());
}

// ── Positions ───────────────────────────────────────────

/// The bug: an emoji earlier in the line shifts every UTF-16 offset after it.
/// Byte column 8 sits after "let x = " in both encodings only when ASCII.
#[test]
fn utf16_columns_account_for_wide_characters() {
    let text = "let 🚀 = 1;";
    // Byte column after "let 🚀 ". The emoji is 4 bytes but 2 UTF-16 units, so
    // the two encodings disagree by exactly the 2-byte surplus.
    let byte_col = "let 🚀 ".len() as i64 + 1; // 10th byte
    let position = to_lsp_position(text, 1, byte_col, PositionEncoding::Utf16);
    assert_eq!(position["character"], 7); // l,e,t,space,🚀(2 units),space
    let position = to_lsp_position(text, 1, byte_col, PositionEncoding::Utf8);
    assert_eq!(position["character"], byte_col - 1);
}

#[test]
fn utf16_positions_convert_back_to_byte_columns() {
    let text = "let 🚀 = 1;";
    let (line, col) = from_lsp_position(
        text,
        &json!({ "line": 0, "character": 7 }),
        PositionEncoding::Utf16,
    );
    assert_eq!(line, 1);
    assert_eq!(col as usize, "let 🚀 ".len() + 1);
}

#[test]
fn positions_are_one_based_for_the_editor() {
    let text = "a\nb\nc";
    let position = to_lsp_position(text, 3, 1, PositionEncoding::Utf8);
    assert_eq!(position["line"], 2);
    assert_eq!(position["character"], 0);
}

/// A column past the end of a line comes from a stale buffer; clamp instead of
/// panicking on a slice.
#[test]
fn out_of_range_columns_clamp() {
    let text = "short";
    let position = to_lsp_position(text, 1, 9_999, PositionEncoding::Utf16);
    assert_eq!(position["character"], 5);
}

// ── Hover ───────────────────────────────────────────────

#[test]
fn hover_reads_markup_content() {
    let result = json!({ "contents": { "kind": "markdown", "value": "fn main()" } });
    assert_eq!(hover_text(&result).unwrap(), "fn main()");
}

#[test]
fn hover_reads_a_plain_string() {
    assert_eq!(hover_text(&json!({ "contents": "i32" })).unwrap(), "i32");
}

#[test]
fn hover_joins_an_array_of_parts() {
    let result = json!({ "contents": ["fn main()", { "value": "the entry point" }] });
    assert_eq!(hover_text(&result).unwrap(), "fn main()\n\nthe entry point");
}

#[test]
fn empty_hover_is_none() {
    assert!(hover_text(&json!({ "contents": "   " })).is_none());
    assert!(hover_text(&json!({})).is_none());
}

// ── Definition ──────────────────────────────────────────

#[test]
fn definition_accepts_a_bare_location() {
    let result = json!({
        "uri": "file:///a.rs",
        "range": { "start": { "line": 4, "character": 2 }, "end": { "line": 4, "character": 6 } }
    });
    let targets = definition_targets(&result);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].0, "file:///a.rs");
    assert_eq!(targets[0].1["line"], 4);
}

/// With `linkSupport` declared, servers may answer with LocationLink, which
/// spells its fields differently. Handling only Location silently drops these.
#[test]
fn definition_accepts_location_links() {
    let result = json!([{
        "targetUri": "file:///b.rs",
        "targetRange": { "start": { "line": 1, "character": 0 }, "end": { "line": 9, "character": 0 } },
        "targetSelectionRange": { "start": { "line": 2, "character": 3 }, "end": { "line": 2, "character": 8 } }
    }]);
    let targets = definition_targets(&result);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].0, "file:///b.rs");
    // The selection range is the name itself, which is where you want to land.
    assert_eq!(targets[0].1["line"], 2);
    assert_eq!(targets[0].1["character"], 3);
}

#[test]
fn definition_handles_no_result() {
    assert!(definition_targets(&Value::Null).is_empty());
    assert!(definition_targets(&json!([])).is_empty());
}

// ── Text edits ──────────────────────────────────────────

/// Edits must apply back-to-front, or the second edit's offsets refer to text
/// the first one already moved.
#[test]
fn edits_apply_without_disturbing_each_other() {
    let text = "aaa\nbbb\nccc\n";
    let edits = vec![
        json!({ "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 3 } }, "newText": "XXXXXX" }),
        json!({ "range": { "start": { "line": 2, "character": 0 }, "end": { "line": 2, "character": 3 } }, "newText": "Z" }),
    ];
    let out = apply_text_edits(text, &edits, PositionEncoding::Utf8).unwrap();
    assert_eq!(out, "XXXXXX\nbbb\nZ\n");
}

#[test]
fn a_whole_document_edit_replaces_everything() {
    let text = "old\n";
    let edits = vec![json!({
        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
        "newText": "new\n"
    })];
    assert_eq!(
        apply_text_edits(text, &edits, PositionEncoding::Utf8).unwrap(),
        "new\n"
    );
}

/// Better to refuse than to write a mangled file over the user's source.
#[test]
fn malformed_edits_are_refused() {
    let edits = vec![json!({ "newText": "x" })];
    assert!(apply_text_edits("abc", &edits, PositionEncoding::Utf8).is_none());
}
