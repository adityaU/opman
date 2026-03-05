//! Markdown table parsing.
//!
//! Parses pipe-delimited markdown tables into structured data with headers,
//! alignment info, and row data.  Used by `parse.rs` when a table is detected.

use super::TableAlign;

/// Parse a markdown table starting at `lines[start]`.
///
/// Returns `(headers, alignments, data_rows, lines_consumed)`.
pub(crate) fn parse_table(
    lines: &[&str],
    start: usize,
) -> (Vec<String>, Vec<TableAlign>, Vec<Vec<String>>, usize) {
    let total = lines.len();
    let mut i = start;

    // Header row
    let headers = parse_row(lines[i]);
    i += 1;

    // Separator row → alignment info
    let alignments = parse_alignments(lines[i]);
    i += 1;

    // Data rows
    let mut rows: Vec<Vec<String>> = Vec::new();
    while i < total && is_table_line(lines[i]) && !is_separator(lines[i]) {
        rows.push(parse_row(lines[i]));
        i += 1;
    }

    (headers, alignments, rows, i - start)
}

/// Split a table row into cell strings.
fn parse_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// Extract column alignments from the separator row.
fn parse_alignments(line: &str) -> Vec<TableAlign> {
    let trimmed = line.trim();
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    inner
        .split('|')
        .map(|cell| {
            let c = cell.trim();
            let left = c.starts_with(':');
            let right = c.ends_with(':');
            match (left, right) {
                (true, true) => TableAlign::Center,
                (false, true) => TableAlign::Right,
                _ => TableAlign::Left,
            }
        })
        .collect()
}

/// Check if a line is a markdown table separator (e.g. `| --- | :---: |`).
pub(crate) fn is_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '|' | '-' | ':' | ' '))
        .collect();
    cleaned.is_empty() && trimmed.contains('-')
}

/// Check if a line looks like part of a markdown table (contains `|`).
pub(crate) fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.contains('|')
}

// ── Segment splitting ─────────────────────────────────────────────────

/// A segment of markdown text, either plain text or a table.
#[derive(Debug)]
pub enum MdTextSegment {
    /// Plain markdown text (no tables).
    Text(String),
    /// A markdown table in its raw text form.
    Table(String),
}

/// Split markdown text into alternating `Text` and `Table` segments.
///
/// This allows callers (e.g. the Slack relay watcher) to handle tables
/// differently from surrounding prose — for instance, finalizing a
/// stream before posting a table as a Block Kit native table block.
///
/// Tables are detected the same way as in `parse.rs`: a line containing
/// `|` followed immediately by a separator row.
pub fn split_around_tables(md: &str) -> Vec<MdTextSegment> {
    let lines: Vec<&str> = md.lines().collect();
    let total = lines.len();
    let mut segments: Vec<MdTextSegment> = Vec::new();
    let mut text_buf: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < total {
        let line = lines[i];

        // Detect table start: line with `|` followed by a separator row.
        if line.contains('|') && i + 1 < total && is_separator(lines[i + 1]) {
            // Flush accumulated text.
            if !text_buf.is_empty() {
                let text = text_buf.join("\n");
                if !text.trim().is_empty() {
                    segments.push(MdTextSegment::Text(text));
                }
                text_buf.clear();
            }

            // Collect all contiguous table rows.
            let table_start = i;
            let mut table_end = i;
            while table_end < total && is_table_line(lines[table_end]) {
                table_end += 1;
            }
            let table_text = lines[table_start..table_end].join("\n");
            segments.push(MdTextSegment::Table(table_text));
            i = table_end;
        } else {
            text_buf.push(line);
            i += 1;
        }
    }

    // Flush remaining text.
    if !text_buf.is_empty() {
        let text = text_buf.join("\n");
        if !text.trim().is_empty() {
            segments.push(MdTextSegment::Text(text));
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_no_table() {
        let md = "Hello world\n\nSome text here.";
        let segs = split_around_tables(md);
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], MdTextSegment::Text(_)));
    }

    #[test]
    fn test_split_table_only() {
        let md = "| A | B |\n| --- | --- |\n| 1 | 2 |";
        let segs = split_around_tables(md);
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], MdTextSegment::Table(_)));
    }

    #[test]
    fn test_split_text_table_text() {
        let md = "Before table\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nAfter table";
        let segs = split_around_tables(md);
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], MdTextSegment::Text(_)));
        assert!(matches!(&segs[1], MdTextSegment::Table(_)));
        assert!(matches!(&segs[2], MdTextSegment::Text(_)));
    }

    #[test]
    fn test_split_multiple_tables() {
        let md = "Text1\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nMiddle\n\n| C | D |\n| --- | --- |\n| 3 | 4 |\n\nEnd";
        let segs = split_around_tables(md);
        assert_eq!(segs.len(), 5);
        assert!(matches!(&segs[0], MdTextSegment::Text(_)));
        assert!(matches!(&segs[1], MdTextSegment::Table(_)));
        assert!(matches!(&segs[2], MdTextSegment::Text(_)));
        assert!(matches!(&segs[3], MdTextSegment::Table(_)));
        assert!(matches!(&segs[4], MdTextSegment::Text(_)));
    }
}
