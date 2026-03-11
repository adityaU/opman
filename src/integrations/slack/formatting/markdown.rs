//! Markdown → Slack mrkdwn conversion helpers.

/// Convert standard Markdown to Slack mrkdwn format.
///
/// Key differences handled:
/// - `**bold**` → `*bold*`
/// - `*italic*` (when not inside bold) → `_italic_`
/// - `~~strike~~` → `~strike~`
/// - `[text](url)` → `<url|text>`
/// - `# heading` → `*heading*` (bold, Slack has no headings)
/// - Markdown tables → fenced code blocks (Slack has no table syntax)
/// - Fenced code blocks and inline code pass through unchanged.
pub fn markdown_to_slack_mrkdwn(md: &str) -> String {
    // Pre-pass: convert markdown tables to code blocks.
    let preprocessed = convert_markdown_tables(md);
    convert_inline_markdown(&preprocessed)
}

/// Detect markdown tables and wrap them in fenced code blocks so they render
/// as monospace in Slack (which has no native table support).
///
/// A markdown table is identified as a sequence of lines where:
/// - Each line contains at least one `|` character
/// - One of the early lines is a separator row (e.g. `|---|---|`)
pub fn convert_markdown_tables(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut result = String::with_capacity(md.len() + 128);
    let mut i = 0;
    let total = lines.len();
    // Track whether the original text ended with a newline.
    let ends_with_newline = md.ends_with('\n');

    while i < total {
        let line = lines[i];

        // Check if this line looks like a table row (contains `|`).
        // Also require the next line to be a separator row for a valid table.
        if line.contains('|') && i + 1 < total && is_table_separator(lines[i + 1]) {
            // Found a table. Collect all contiguous table rows.
            let table_start = i;
            let mut table_end = i;
            while table_end < total && is_table_line(lines[table_end]) {
                table_end += 1;
            }

            // Check if already inside a code fence (crude check: look back for
            // unmatched ```). We skip conversion if so.
            let preceding = &result;
            let fence_count = preceding.matches("```").count();
            if fence_count % 2 == 1 {
                // Inside a code block — pass through as-is.
                for j in table_start..table_end {
                    result.push_str(lines[j]);
                    result.push('\n');
                }
            } else {
                result.push_str("```\n");
                for j in table_start..table_end {
                    result.push_str(lines[j]);
                    result.push('\n');
                }
                result.push_str("```\n");
            }
            i = table_end;
        } else {
            result.push_str(line);
            if i + 1 < total || ends_with_newline {
                result.push('\n');
            }
            i += 1;
        }
    }

    result
}

/// Check if a line is a markdown table separator row (e.g. `| --- | --- |` or `|:---:|---:|`).
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }
    // After removing `|`, `:`, `-`, and whitespace, nothing should remain.
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '|' | '-' | ':' | ' '))
        .collect();
    cleaned.is_empty() && trimmed.contains('-')
}

/// Check if a line looks like part of a markdown table (contains `|`).
fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    // A table line should contain `|`. Empty lines break the table.
    !trimmed.is_empty() && trimmed.contains('|')
}

/// The character-level inline markdown to Slack mrkdwn converter.
fn convert_inline_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut chars = md.chars().peekable();
    // Track whether we are inside a fenced code block (```).
    let mut in_fenced_block = false;
    // Track whether we are inside an inline code span (`).
    let mut in_inline_code = false;
    // Track whether we are at the start of a line (for heading detection).
    let mut at_line_start = true;

    while let Some(c) = chars.next() {
        // ── Fenced code block toggle ───────────────────────────────
        if c == '`' && chars.peek() == Some(&'`') {
            // Check for triple backtick.
            let second = chars.next(); // consume 2nd `
            if chars.peek() == Some(&'`') {
                let third = chars.next(); // consume 3rd `
                in_fenced_block = !in_fenced_block;
                result.push(c);
                if let Some(ch) = second {
                    result.push(ch);
                }
                if let Some(ch) = third {
                    result.push(ch);
                }
                at_line_start = false;
                continue;
            } else {
                // Only two backticks — not a fence, push them through.
                result.push(c);
                if let Some(ch) = second {
                    result.push(ch);
                }
                at_line_start = false;
                continue;
            }
        }

        // Inside fenced code blocks, pass everything through verbatim.
        if in_fenced_block {
            if c == '\n' {
                at_line_start = true;
            } else {
                at_line_start = false;
            }
            result.push(c);
            continue;
        }

        // ── Inline code toggle ─────────────────────────────────────
        if c == '`' {
            in_inline_code = !in_inline_code;
            result.push(c);
            at_line_start = false;
            continue;
        }

        // Inside inline code, pass through verbatim.
        if in_inline_code {
            result.push(c);
            at_line_start = false;
            continue;
        }

        // ── Headings at line start ─────────────────────────────────
        if c == '#' && at_line_start {
            // Consume all leading '#' and optional space.
            while chars.peek() == Some(&'#') {
                chars.next();
            }
            if chars.peek() == Some(&' ') {
                chars.next();
            }
            // Collect the heading text until end of line.
            let mut heading = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '\n' {
                    break;
                }
                heading.push(chars.next().unwrap());
            }
            // Render as bold in Slack.
            result.push('*');
            result.push_str(heading.trim());
            result.push('*');
            at_line_start = false;
            continue;
        }

        // ── Bold: **text** → *text* ────────────────────────────────
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            result.push('*');
            at_line_start = false;
            continue;
        }

        // ── Italic: standalone *text* → _text_ ────────────────────
        // At this point a single `*` that wasn't part of `**` is italic.
        if c == '*' {
            result.push('_');
            at_line_start = false;
            continue;
        }

        // ── Strikethrough: ~~text~~ → ~text~ ───────────────────────
        if c == '~' && chars.peek() == Some(&'~') {
            chars.next(); // consume second ~
            result.push('~');
            at_line_start = false;
            continue;
        }

        // ── Links: [text](url) → <url|text> ───────────────────────
        if c == '[' {
            // Try to parse a markdown link.
            let mut link_text = String::new();
            let mut found_link = false;
            let mut inner_chars = chars.clone();
            let mut bracket_depth = 1;

            // Collect text inside brackets.
            while let Some(ch) = inner_chars.next() {
                if ch == '[' {
                    bracket_depth += 1;
                } else if ch == ']' {
                    bracket_depth -= 1;
                    if bracket_depth == 0 {
                        // Check if followed by (url)
                        if inner_chars.peek() == Some(&'(') {
                            inner_chars.next(); // consume '('
                            let mut url = String::new();
                            let mut paren_depth = 1;
                            while let Some(uc) = inner_chars.next() {
                                if uc == '(' {
                                    paren_depth += 1;
                                    url.push(uc);
                                } else if uc == ')' {
                                    paren_depth -= 1;
                                    if paren_depth == 0 {
                                        found_link = true;
                                        break;
                                    }
                                    url.push(uc);
                                } else {
                                    url.push(uc);
                                }
                            }
                            if found_link {
                                result.push('<');
                                result.push_str(&url);
                                result.push('|');
                                result.push_str(&link_text);
                                result.push('>');
                                chars = inner_chars;
                            }
                        }
                        break;
                    }
                }
                if bracket_depth > 0 {
                    link_text.push(ch);
                }
            }
            if !found_link {
                result.push(c);
            }
            at_line_start = false;
            continue;
        }

        // ── Newline tracking ───────────────────────────────────────
        if c == '\n' {
            at_line_start = true;
            result.push(c);
            continue;
        }

        at_line_start = false;
        result.push(c);
    }

    result
}
