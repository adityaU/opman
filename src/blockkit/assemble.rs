//! Assemble `MdSegment`s into Slack Block Kit blocks.
//!
//! Strategy: reconstruct the original markdown from parsed segments and emit
//! the entire response as a **single `markdown` block**.  This mirrors how
//! Slack's own AI features render responses and is required for fenced code
//! block syntax highlighting to work.
//!
//! Tables are the exception — they are emitted as native Slack `table` blocks
//! (sent via `attachments`).  When a table is encountered the accumulated
//! markdown text is flushed as a `markdown` block, the table block is emitted,
//! and accumulation resumes for subsequent content.

use serde_json::{json, Value};

use super::parse::MdSegment;

/// Convert parsed markdown segments into Slack Block Kit blocks.
///
/// Non-table segments are reconstructed back into markdown text and emitted
/// as `{"type": "markdown", "text": "..."}` blocks.  Tables are emitted as
/// native `table` blocks (the caller separates them into `attachments`).
pub(crate) fn assemble_blocks(segments: &[MdSegment<'_>]) -> Vec<Value> {
    let mut blocks: Vec<Value> = Vec::new();
    let mut md_buf = String::new();

    for segment in segments {
        match segment {
            MdSegment::Table {
                headers,
                alignments,
                rows,
            } => {
                // Flush accumulated markdown before the table.
                flush_markdown(&mut md_buf, &mut blocks);
                blocks.push(build_table_block(headers, alignments, rows));
            }
            _ => {
                // Reconstruct markdown from the segment and append.
                if !md_buf.is_empty() {
                    md_buf.push_str("\n\n");
                }
                segment_to_markdown(segment, &mut md_buf);
            }
        }
    }

    // Flush any remaining markdown.
    flush_markdown(&mut md_buf, &mut blocks);

    blocks
}

// ── Markdown reconstruction ─────────────────────────────────────────────

/// Flush accumulated markdown text into a single `markdown` block.
fn flush_markdown(buf: &mut String, blocks: &mut Vec<Value>) {
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        buf.clear();
        return;
    }
    blocks.push(json!({
        "type": "markdown",
        "text": trimmed
    }));
    buf.clear();
}

/// Reconstruct markdown source text from a single `MdSegment` and append it
/// to `buf`.  This is intentionally lossy-free for the segment types we care
/// about (headings, code blocks, lists, etc.).
fn segment_to_markdown(segment: &MdSegment<'_>, buf: &mut String) {
    match segment {
        MdSegment::Heading1(text) => {
            buf.push_str("# ");
            buf.push_str(text);
        }

        MdSegment::Heading3(text, level) => {
            for _ in 0..*level {
                buf.push('#');
            }
            buf.push(' ');
            buf.push_str(text);
        }

        MdSegment::Paragraph(lines) => {
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                buf.push_str(line);
            }
        }

        MdSegment::CodeBlock { lang, code } => {
            buf.push_str("```");
            if let Some(l) = lang {
                buf.push_str(l);
            }
            buf.push('\n');
            buf.push_str(code);
            buf.push_str("\n```");
        }

        MdSegment::Blockquote(lines) => {
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                buf.push_str("> ");
                buf.push_str(line);
            }
        }

        MdSegment::BulletList(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                // Reconstruct indentation.
                for _ in 0..item.indent {
                    buf.push_str("  ");
                }
                buf.push_str("- ");
                buf.push_str(&item.text);
            }
        }

        MdSegment::OrderedList(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                for _ in 0..item.indent {
                    buf.push_str("  ");
                }
                // Use 1-based numbering.
                buf.push_str(&format!("{}. ", i + 1));
                buf.push_str(&item.text);
            }
        }

        MdSegment::TodoList(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    buf.push('\n');
                }
                if item.checked {
                    buf.push_str("- [x] ");
                } else {
                    buf.push_str("- [ ] ");
                }
                buf.push_str(&item.text);
            }
        }

        MdSegment::HorizontalRule => {
            buf.push_str("---");
        }

        // Tables are handled separately in assemble_blocks(); this arm
        // should never be reached.
        MdSegment::Table { .. } => {}
    }
}

// ── Table block builder ─────────────────────────────────────────────────

/// Build a native Slack `table` block from parsed markdown table data.
///
/// The Slack table block format:
/// - `rows`: array of row arrays.  First row = header.  Each cell is
///   `{"type": "raw_text", "text": "..."}` or a `rich_text` element.
/// - `column_settings`: optional array with `align` and `is_wrapped`.
/// - Max 100 rows, max 20 columns.
fn build_table_block(
    headers: &[String],
    alignments: &[super::TableAlign],
    rows: &[Vec<String>],
) -> Value {
    let mut all_rows: Vec<Value> = Vec::with_capacity(rows.len() + 1);

    // Header row (first row in Slack's table block acts as the header).
    let header_cells: Vec<Value> = headers
        .iter()
        .take(20) // max 20 columns
        .map(|h| json!({ "type": "raw_text", "text": h }))
        .collect();
    all_rows.push(Value::Array(header_cells));

    // Data rows (max 99 data rows + 1 header = 100 total).
    for row in rows.iter().take(99) {
        let cells: Vec<Value> = row
            .iter()
            .take(20)
            .map(|cell| json!({ "type": "raw_text", "text": cell }))
            .collect();
        all_rows.push(Value::Array(cells));
    }

    // Column settings (alignment).
    let col_settings: Vec<Value> = alignments
        .iter()
        .take(20)
        .map(|a| {
            let align_str = match a {
                super::TableAlign::Left => "left",
                super::TableAlign::Center => "center",
                super::TableAlign::Right => "right",
            };
            json!({ "align": align_str, "is_wrapped": true })
        })
        .collect();

    let mut block = json!({
        "type": "table",
        "rows": all_rows,
    });
    if !col_settings.is_empty() {
        block["column_settings"] = json!(col_settings);
    }

    block
}
