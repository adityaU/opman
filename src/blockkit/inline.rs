//! Inline markdown → Block Kit `rich_text` element parser.
//!
//! Handles: `**bold**`, `*italic*`, `` `code` ``, `~~strike~~`, `[text](url)`.

use super::InlineStyle;

/// Parse inline markdown into a list of `rich_text` element JSON values.
pub(crate) fn parse_inline_elements(text: &str) -> Vec<serde_json::Value> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut elements: Vec<serde_json::Value> = Vec::new();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // ── Link: [text](url)
        if chars[i] == '[' {
            if let Some((link_text, url, consumed)) = try_parse_link(&chars, i) {
                flush_buf(&mut buf, &mut elements);
                elements.push(link_element(&url, &link_text));
                i += consumed;
                continue;
            }
        }

        // ── Inline code: `code`
        if chars[i] == '`' && !peek_is(&chars, i + 1, '`') {
            if let Some((code_text, consumed)) =
                try_parse_delimited(&chars, i, '`', '`', false)
            {
                flush_buf(&mut buf, &mut elements);
                elements.push(text_element(&code_text, Some(InlineStyle::Code)));
                i += consumed;
                continue;
            }
        }

        // ── Bold: **text**
        if chars[i] == '*' && peek_is(&chars, i + 1, '*') {
            if let Some((inner, consumed)) = try_parse_double_delimited(&chars, i, '*') {
                flush_buf(&mut buf, &mut elements);
                elements.push(text_element(&inner, Some(InlineStyle::Bold)));
                i += consumed;
                continue;
            }
        }

        // ── Strikethrough: ~~text~~
        if chars[i] == '~' && peek_is(&chars, i + 1, '~') {
            if let Some((inner, consumed)) = try_parse_double_delimited(&chars, i, '~') {
                flush_buf(&mut buf, &mut elements);
                elements.push(text_element(&inner, Some(InlineStyle::Strike)));
                i += consumed;
                continue;
            }
        }

        // ── Italic: *text* (single, not double)
        if chars[i] == '*'
            && !peek_is(&chars, i + 1, '*')
            && !peek_is(&chars, i + 1, ' ')
        {
            if let Some((inner, consumed)) =
                try_parse_delimited(&chars, i, '*', '*', true)
            {
                flush_buf(&mut buf, &mut elements);
                elements.push(text_element(&inner, Some(InlineStyle::Italic)));
                i += consumed;
                continue;
            }
        }

        buf.push(chars[i]);
        i += 1;
    }

    flush_buf(&mut buf, &mut elements);

    if elements.is_empty() {
        elements.push(text_element("", None));
    }
    elements
}

// ── Element constructors ────────────────────────────────────────────────

pub(crate) fn text_element(text: &str, style: Option<InlineStyle>) -> serde_json::Value {
    let mut el = serde_json::json!({ "type": "text", "text": text });
    match style {
        Some(InlineStyle::Bold) => {
            el["style"] = serde_json::json!({ "bold": true });
        }
        Some(InlineStyle::Italic) => {
            el["style"] = serde_json::json!({ "italic": true });
        }
        Some(InlineStyle::Code) => {
            el["style"] = serde_json::json!({ "code": true });
        }
        Some(InlineStyle::Strike) => {
            el["style"] = serde_json::json!({ "strike": true });
        }
        Some(InlineStyle::BoldItalic) => {
            el["style"] = serde_json::json!({ "bold": true, "italic": true });
        }
        None => {}
    }
    el
}

pub(crate) fn link_element(url: &str, text: &str) -> serde_json::Value {
    serde_json::json!({ "type": "link", "url": url, "text": text })
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn flush_buf(buf: &mut String, elements: &mut Vec<serde_json::Value>) {
    if !buf.is_empty() {
        elements.push(text_element(buf, None));
        buf.clear();
    }
}

fn peek_is(chars: &[char], idx: usize, expected: char) -> bool {
    chars.get(idx).copied() == Some(expected)
}

/// Try to parse `[text](url)` starting at `i` (which should be `[`).
fn try_parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if chars.get(start).copied() != Some('[') {
        return None;
    }
    let len = chars.len();
    let mut j = start + 1;
    let mut depth = 1;
    while j < len && depth > 0 {
        match chars[j] {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            j += 1;
        }
    }
    if depth != 0 || j >= len {
        return None;
    }
    let text: String = chars[start + 1..j].iter().collect();
    if chars.get(j + 1).copied() != Some('(') {
        return None;
    }
    let url_start = j + 2;
    let mut k = url_start;
    let mut paren_depth = 1;
    while k < len && paren_depth > 0 {
        match chars[k] {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            _ => {}
        }
        if paren_depth > 0 {
            k += 1;
        }
    }
    if paren_depth != 0 {
        return None;
    }
    let url: String = chars[url_start..k].iter().collect();
    Some((text, url, k + 1 - start))
}

/// Parse a single-char delimited span: `d…d`.
fn try_parse_delimited(
    chars: &[char],
    start: usize,
    open: char,
    close: char,
    skip_spaces: bool,
) -> Option<(String, usize)> {
    let len = chars.len();
    if chars.get(start).copied() != Some(open) {
        return None;
    }
    let mut j = start + 1;
    while j < len {
        if chars[j] == close {
            if skip_spaces && j > start + 1 && chars[j - 1] == ' ' {
                j += 1;
                continue;
            }
            let inner: String = chars[start + 1..j].iter().collect();
            if inner.is_empty() {
                return None;
            }
            return Some((inner, j + 1 - start));
        }
        if chars[j] == '\n' {
            return None;
        }
        j += 1;
    }
    None
}

/// Parse a double-char delimited span: `dd…dd` (e.g., `**…**`, `~~…~~`).
fn try_parse_double_delimited(
    chars: &[char],
    start: usize,
    delim: char,
) -> Option<(String, usize)> {
    let len = chars.len();
    if start + 1 >= len || chars[start] != delim || chars[start + 1] != delim {
        return None;
    }
    let mut j = start + 2;
    while j + 1 < len {
        if chars[j] == delim && chars[j + 1] == delim {
            let inner: String = chars[start + 2..j].iter().collect();
            if inner.is_empty() {
                return None;
            }
            return Some((inner, j + 2 - start));
        }
        if chars[j] == '\n' && j + 1 < len && chars[j + 1] == '\n' {
            return None;
        }
        j += 1;
    }
    None
}
