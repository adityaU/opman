//! Block-level markdown parser.
//!
//! Splits markdown text into a sequence of block-level segments: headings,
//! fenced code blocks, tables, blockquotes, lists, todo lists, horizontal
//! rules, and prose paragraphs.

use super::table;
use super::{ListItem, TableAlign, TodoItem};

/// A block-level segment parsed from markdown text.
#[derive(Debug)]
pub enum MdSegment<'a> {
    /// `#` or `##` heading → `header` block.
    Heading1(String),
    /// `###`+ heading → bold text in `rich_text_section`.
    Heading3(String, u8),
    /// Prose paragraph (may contain inline formatting).
    Paragraph(Vec<&'a str>),
    /// Fenced code block with optional language annotation.
    CodeBlock { lang: Option<String>, code: String },
    /// Markdown table.
    Table {
        headers: Vec<String>,
        alignments: Vec<TableAlign>,
        rows: Vec<Vec<String>>,
    },
    /// Blockquote (lines prefixed with `> `).
    Blockquote(Vec<&'a str>),
    /// Unordered (bullet) list.
    BulletList(Vec<ListItem>),
    /// Ordered (numbered) list.
    OrderedList(Vec<ListItem>),
    /// Checkbox / todo list.
    TodoList(Vec<TodoItem>),
    /// Horizontal rule (`---`, `***`, `___`).
    HorizontalRule,
}

/// Parse markdown text into a sequence of block-level segments.
pub(crate) fn parse_block_segments(md: &str) -> Vec<MdSegment<'_>> {
    let lines: Vec<&str> = md.lines().collect();
    let total = lines.len();
    let mut segments: Vec<MdSegment<'_>> = Vec::new();
    let mut i = 0;

    while i < total {
        let line = lines[i];
        let trimmed = line.trim();

        // ── Blank line ──────────────────────────────────────────────
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // ── Horizontal rule ─────────────────────────────────────────
        if is_horizontal_rule(trimmed) {
            segments.push(MdSegment::HorizontalRule);
            i += 1;
            continue;
        }

        // ── Heading ─────────────────────────────────────────────────
        if trimmed.starts_with('#') {
            if let Some((level, text)) = parse_heading_line(trimmed) {
                if level <= 2 {
                    segments.push(MdSegment::Heading1(text));
                } else {
                    segments.push(MdSegment::Heading3(text, level));
                }
                i += 1;
                continue;
            }
        }

        // ── Fenced code block ───────────────────────────────────────
        if trimmed.starts_with("```") {
            let lang = {
                let after = trimmed.trim_start_matches('`').trim();
                if after.is_empty() { None } else { Some(after.to_string()) }
            };
            i += 1;
            let mut code_lines: Vec<&str> = Vec::new();
            while i < total {
                if lines[i].trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            segments.push(MdSegment::CodeBlock {
                lang,
                code: code_lines.join("\n"),
            });
            continue;
        }

        // ── Table ───────────────────────────────────────────────────
        if line.contains('|')
            && i + 1 < total
            && table::is_separator(lines[i + 1])
        {
            let (headers, alignments, rows, consumed) = table::parse_table(&lines, i);
            segments.push(MdSegment::Table {
                headers,
                alignments,
                rows,
            });
            i += consumed;
            continue;
        }

        // ── Blockquote ──────────────────────────────────────────────
        if trimmed.starts_with("> ") || trimmed == ">" {
            let start = i;
            while i < total
                && (lines[i].trim().starts_with("> ") || lines[i].trim() == ">")
            {
                i += 1;
            }
            let quote_lines: Vec<&str> = lines[start..i]
                .iter()
                .map(|l| {
                    let t = l.trim();
                    if t == ">" { "" } else { t.strip_prefix("> ").unwrap_or(t) }
                })
                .collect();
            segments.push(MdSegment::Blockquote(quote_lines));
            continue;
        }

        // ── Todo list ───────────────────────────────────────────────
        if is_todo_line(trimmed) {
            let mut items: Vec<TodoItem> = Vec::new();
            while i < total && is_todo_line(lines[i].trim()) {
                let t = lines[i].trim();
                let checked = t.contains("[x]") || t.contains("[X]");
                let text = t
                    .trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace())
                    .trim_start_matches("[x]")
                    .trim_start_matches("[X]")
                    .trim_start_matches("[ ]")
                    .trim()
                    .to_string();
                items.push(TodoItem { text, checked });
                i += 1;
            }
            segments.push(MdSegment::TodoList(items));
            continue;
        }

        // ── Bullet list ─────────────────────────────────────────────
        if is_bullet_line(trimmed) {
            let mut items: Vec<ListItem> = Vec::new();
            while i < total && is_bullet_line(lines[i].trim_end()) {
                let raw = lines[i];
                let indent = raw.len() - raw.trim_start().len();
                let text = raw
                    .trim()
                    .trim_start_matches(|c: char| c == '-' || c == '*' || c == '+')
                    .trim_start()
                    .to_string();
                items.push(ListItem { text, indent: indent / 2 });
                i += 1;
            }
            segments.push(MdSegment::BulletList(items));
            continue;
        }

        // ── Ordered list ────────────────────────────────────────────
        if is_ordered_line(trimmed) {
            let mut items: Vec<ListItem> = Vec::new();
            while i < total && is_ordered_line(lines[i].trim_end()) {
                let raw = lines[i];
                let indent = raw.len() - raw.trim_start().len();
                let text = raw
                    .trim()
                    .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')')
                    .trim_start()
                    .to_string();
                items.push(ListItem { text, indent: indent / 2 });
                i += 1;
            }
            segments.push(MdSegment::OrderedList(items));
            continue;
        }

        // ── Prose paragraph (fallback) ──────────────────────────────
        {
            let start = i;
            while i < total {
                let l = lines[i].trim();
                if l.is_empty()
                    || l.starts_with('#')
                    || l.starts_with("```")
                    || l.starts_with("> ")
                    || is_horizontal_rule(l)
                    || is_todo_line(l)
                    || is_bullet_line(l)
                    || is_ordered_line(l)
                    || (l.contains('|')
                        && i + 1 < total
                        && table::is_separator(lines.get(i + 1).unwrap_or(&"")))
                {
                    break;
                }
                i += 1;
            }
            segments.push(MdSegment::Paragraph(lines[start..i].to_vec()));
        }
    }

    segments
}

// ── Helper predicates ───────────────────────────────────────────────────

fn is_horizontal_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3
        && (t.chars().all(|c| c == '-' || c == ' ')
            || t.chars().all(|c| c == '*' || c == ' ')
            || t.chars().all(|c| c == '_' || c == ' '))
        && t.chars().filter(|c| !c.is_whitespace()).count() >= 3
}

fn parse_heading_line(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = trimmed[level..].trim().trim_end_matches('#').trim();
    if rest.is_empty() {
        return None;
    }
    Some((level as u8, rest.to_string()))
}

fn is_todo_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("- [ ] ")
        || t.starts_with("- [x] ")
        || t.starts_with("- [X] ")
        || t.starts_with("* [ ] ")
        || t.starts_with("* [x] ")
        || t.starts_with("* [X] ")
}

fn is_bullet_line(line: &str) -> bool {
    let t = line.trim();
    if is_todo_line(t) || is_horizontal_rule(t) {
        return false;
    }
    t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ")
}

fn is_ordered_line(line: &str) -> bool {
    let t = line.trim();
    let mut chars = t.chars();
    let first = chars.next();
    if !first.map_or(false, |c| c.is_ascii_digit()) {
        return false;
    }
    let rest: String = chars.collect();
    rest.starts_with(". ")
        || rest.starts_with(") ")
        || (rest.chars().next().map_or(false, |c| c.is_ascii_digit())
            && (rest.contains(". ") || rest.contains(") ")))
}
