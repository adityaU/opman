//! Translation between the protocol's conventions and the editor's.
//!
//! Three mismatches, each a silent-wrong-answer bug if skipped:
//!
//! * Paths travel as `file://` URIs with percent-encoding.
//! * LSP lines and characters are 0-based; the editor's are 1-based.
//! * An LSP `character` counts **UTF-16 code units**, not bytes and not
//!   characters. On a line containing an emoji or an accent, subtracting one
//!   from a byte column lands on a different token and hover describes the
//!   wrong symbol — confidently. Servers that accept `utf-8` are told to use
//!   it; the conversion exists for the ones that will not.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// Characters that must survive unescaped in a path segment.
fn needs_escaping(byte: u8) -> bool {
    !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/'))
}

pub fn path_to_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().as_bytes() {
        if needs_escaping(*byte) {
            uri.push_str(&format!("%{byte:02X}"));
        } else {
            uri.push(*byte as char);
        }
    }
    uri
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

// ── Positions ───────────────────────────────────────────

/// Whether the server wants UTF-16 code units (the protocol default) or bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PositionEncoding {
    #[default]
    Utf16,
    Utf8,
}

impl PositionEncoding {
    pub fn from_server(value: Option<&str>) -> Self {
        match value {
            Some("utf-8") => Self::Utf8,
            _ => Self::Utf16,
        }
    }
}

/// Editor position (1-based line and byte column) → LSP position.
pub fn to_lsp_position(text: &str, line: i64, col: i64, encoding: PositionEncoding) -> Value {
    let line_index = (line - 1).max(0);
    let byte_col = (col - 1).max(0) as usize;
    let character = match encoding {
        PositionEncoding::Utf8 => byte_col as i64,
        PositionEncoding::Utf16 => {
            let source = nth_line(text, line_index as usize).unwrap_or("");
            utf16_units_before(source, byte_col) as i64
        }
    };
    json!({ "line": line_index, "character": character })
}

/// LSP position → editor position (1-based line and byte column).
pub fn from_lsp_position(text: &str, position: &Value, encoding: PositionEncoding) -> (i64, i64) {
    let line = position.get("line").and_then(Value::as_i64).unwrap_or(0);
    let character = position
        .get("character")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0) as usize;
    let col = match encoding {
        PositionEncoding::Utf8 => character,
        PositionEncoding::Utf16 => {
            let source = nth_line(text, line as usize).unwrap_or("");
            bytes_for_utf16_units(source, character)
        }
    };
    (line + 1, col as i64 + 1)
}

fn nth_line(text: &str, index: usize) -> Option<&str> {
    text.split('\n')
        .nth(index)
        .map(|l| l.trim_end_matches('\r'))
}

/// How many UTF-16 code units precede `byte_col` in `line`.
fn utf16_units_before(line: &str, byte_col: usize) -> usize {
    // A column landing mid-character can only come from a stale buffer; round
    // down to the nearest boundary rather than panicking on the slice.
    let mut cut = byte_col.min(line.len());
    while cut > 0 && !line.is_char_boundary(cut) {
        cut -= 1;
    }
    line[..cut].chars().map(char::len_utf16).sum()
}

/// The byte offset that `units` UTF-16 code units reach in `line`.
fn bytes_for_utf16_units(line: &str, units: usize) -> usize {
    let mut seen = 0usize;
    for (offset, ch) in line.char_indices() {
        if seen >= units {
            return offset;
        }
        seen += ch.len_utf16();
    }
    line.len()
}

// ── Payloads ────────────────────────────────────────────

/// Flatten a hover result's `contents` into plain text.
///
/// The field has accumulated three shapes across protocol versions — a marked
/// string, an array of them, or a `MarkupContent` — and servers still use all
/// three.
pub fn hover_text(result: &Value) -> Option<String> {
    let contents = result.get("contents")?;
    let text = match contents {
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(s.clone()),
                Value::Object(map) => map.get("value").and_then(Value::as_str).map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => return None,
    };
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Normalise every shape `textDocument/definition` may return into
/// `(uri, position)` pairs.
///
/// With `linkSupport` declared, a server may answer with a single `Location`,
/// an array of them, or an array of `LocationLink` — which spells its fields
/// differently. Handling only the first two is a common and quiet bug.
pub fn definition_targets(result: &Value) -> Vec<(String, Value)> {
    let items = match result {
        Value::Array(items) => items.clone(),
        Value::Object(_) => vec![result.clone()],
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(uri) = item.get("uri").and_then(Value::as_str) {
                let start = item.get("range")?.get("start")?.clone();
                return Some((uri.to_string(), start));
            }
            let uri = item.get("targetUri").and_then(Value::as_str)?;
            let range = item
                .get("targetSelectionRange")
                .or_else(|| item.get("targetRange"))?;
            Some((uri.to_string(), range.get("start")?.clone()))
        })
        .collect()
}

/// Apply `TextEdit`s to `text`, last edit first so earlier offsets stay valid.
pub fn apply_text_edits(text: &str, edits: &[Value], encoding: PositionEncoding) -> Option<String> {
    let mut resolved: Vec<(usize, usize, String)> = edits
        .iter()
        .filter_map(|edit| {
            let range = edit.get("range")?;
            let new_text = edit.get("newText").and_then(Value::as_str).unwrap_or("");
            let start = offset_of(text, range.get("start")?, encoding)?;
            let end = offset_of(text, range.get("end")?, encoding)?;
            Some((start, end, new_text.to_string()))
        })
        .collect();
    if resolved.len() != edits.len() {
        return None;
    }
    resolved.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));

    let mut out = text.to_string();
    for (start, end, new_text) in resolved {
        if start > end
            || end > out.len()
            || !out.is_char_boundary(start)
            || !out.is_char_boundary(end)
        {
            return None;
        }
        out.replace_range(start..end, &new_text);
    }
    Some(out)
}

/// Absolute byte offset of an LSP position within `text`.
fn offset_of(text: &str, position: &Value, encoding: PositionEncoding) -> Option<usize> {
    let line = position.get("line").and_then(Value::as_i64)? as usize;
    let character = position.get("character").and_then(Value::as_i64)?.max(0) as usize;

    let mut offset = 0usize;
    for (index, source) in text.split_inclusive('\n').enumerate() {
        if index == line {
            let bare = source.trim_end_matches(['\n', '\r']);
            let within = match encoding {
                PositionEncoding::Utf8 => character.min(bare.len()),
                PositionEncoding::Utf16 => bytes_for_utf16_units(bare, character),
            };
            return Some(offset + within);
        }
        offset += source.len();
    }
    // A position one past the last line addresses end-of-document.
    (line >= text.split_inclusive('\n').count()).then_some(text.len())
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod convert_tests;
