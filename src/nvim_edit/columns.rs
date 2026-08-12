//! The only boundary between browser UTF-16 columns and Neovim byte columns.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColumnError {
    NotOnUtf8Boundary,
    NotOnUtf16Boundary,
    OutOfRange,
}

impl fmt::Display for ColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotOnUtf8Boundary => "Neovim column is not on a UTF-8 boundary",
            Self::NotOnUtf16Boundary => "CodeMirror column splits a UTF-16 surrogate pair",
            Self::OutOfRange => "column is outside the line",
        })
    }
}

impl std::error::Error for ColumnError {}

/// Convert a CodeMirror/JavaScript UTF-16 offset into Neovim's byte column.
pub(crate) fn utf16_to_byte(line: &str, column: usize) -> Result<usize, ColumnError> {
    let mut utf16 = 0usize;
    for (byte, character) in line.char_indices() {
        if utf16 == column {
            return Ok(byte);
        }
        let width = character.len_utf16();
        if column < utf16.saturating_add(width) {
            return Err(ColumnError::NotOnUtf16Boundary);
        }
        utf16 += width;
    }
    (utf16 == column)
        .then_some(line.len())
        .ok_or(ColumnError::OutOfRange)
}

/// Convert Neovim's UTF-8 byte column into a CodeMirror UTF-16 offset.
pub(crate) fn byte_to_utf16(line: &str, column: usize) -> Result<usize, ColumnError> {
    if column > line.len() || !line.is_char_boundary(column) {
        return Err(if column > line.len() {
            ColumnError::OutOfRange
        } else {
            ColumnError::NotOnUtf8Boundary
        });
    }
    Ok(line[..column].chars().map(char::len_utf16).sum())
}

#[cfg(test)]
mod tests {
    use super::{byte_to_utf16, utf16_to_byte, ColumnError};

    #[test]
    fn emoji_cjk_and_combining_marks_round_trip() {
        let line = "😀界e\u{301}x";
        for byte in [0, 4, 7, 8, 10, 11] {
            let utf16 = byte_to_utf16(line, byte).expect("valid byte boundary");
            assert_eq!(utf16_to_byte(line, utf16), Ok(byte));
        }
        assert_eq!(utf16_to_byte(line, 1), Err(ColumnError::NotOnUtf16Boundary));
        assert_eq!(byte_to_utf16(line, 1), Err(ColumnError::NotOnUtf8Boundary));
    }

    #[test]
    fn offsets_beyond_the_line_are_rejected() {
        assert_eq!(utf16_to_byte("abc", 4), Err(ColumnError::OutOfRange));
        assert_eq!(byte_to_utf16("abc", 4), Err(ColumnError::OutOfRange));
    }
}
