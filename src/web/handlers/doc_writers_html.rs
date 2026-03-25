//! HTML sanitization and block/run parsing for docx writing.

// ── HTML sanitization ───────────────────────────────────────────────

/// Sanitize/normalize HTML from contenteditable before docx conversion.
/// Allows only safe block and inline elements; strips everything else.
pub fn sanitize_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    let bytes = html.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = html[i..].find('>') {
                let tag_raw = &html[i + 1..i + end];
                let is_close = tag_raw.starts_with('/');
                let tag_body = if is_close { &tag_raw[1..] } else { tag_raw };
                let tag_name = tag_body
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                if is_allowed_tag(&tag_name) {
                    if is_close {
                        out.push_str(&format!("</{tag_name}>"));
                    } else if tag_raw.ends_with('/') {
                        out.push_str(&format!("<{tag_name}/>"));
                    } else {
                        out.push_str(&format!("<{tag_name}>"));
                    }
                }
                i += end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn is_allowed_tag(tag: &str) -> bool {
    matches!(
        tag,
        "h1" | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "p"
            | "br"
            | "b"
            | "strong"
            | "i"
            | "em"
            | "u"
            | "s"
            | "strike"
            | "del"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "tr"
            | "th"
            | "td"
            | "thead"
            | "tbody"
            | "div"
            | "span"
    )
}

// ── Block / run parsing ─────────────────────────────────────────────

pub struct BlockInfo {
    pub text: String,
    pub heading_level: Option<u8>,
    pub runs: Vec<RunInfo>,
}

pub struct RunInfo {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

/// Parse sanitized HTML into block-level elements with inline run info.
pub fn parse_blocks(html: &str) -> Vec<BlockInfo> {
    let mut blocks = Vec::new();
    let mut remaining = html;

    while let Some(start) = remaining.find('<') {
        let prefix = remaining[..start].trim();
        if !prefix.is_empty() {
            blocks.push(BlockInfo {
                text: strip_inline_tags(prefix),
                heading_level: None,
                runs: parse_runs(prefix),
            });
        }
        let Some(end) = remaining[start..].find('>') else {
            break;
        };
        let tag_full = &remaining[start + 1..start + end];
        if tag_full.starts_with('/') {
            remaining = &remaining[start + end + 1..];
            continue;
        }
        let tag = tag_full
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        let heading = heading_level_from_tag(&tag);
        if is_block_tag(&tag) {
            let after_open = &remaining[start + end + 1..];
            let close_tag = format!("</{tag}>");
            if let Some(close_pos) = after_open.find(&close_tag) {
                let inner = &after_open[..close_pos];
                let runs = parse_runs(inner);
                blocks.push(BlockInfo {
                    text: strip_inline_tags(inner),
                    heading_level: heading,
                    runs,
                });
                remaining = &after_open[close_pos + close_tag.len()..];
                continue;
            }
        }
        remaining = &remaining[start + end + 1..];
    }
    let trailing = remaining.trim();
    if !trailing.is_empty() {
        blocks.push(BlockInfo {
            text: strip_inline_tags(trailing),
            heading_level: None,
            runs: parse_runs(trailing),
        });
    }
    blocks
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "li" | "div"
    )
}

fn heading_level_from_tag(tag: &str) -> Option<u8> {
    match tag {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

/// Parse inline formatting runs from an HTML fragment.
fn parse_runs(html: &str) -> Vec<RunInfo> {
    let mut runs = Vec::new();
    let mut bold = false;
    let mut italic = false;
    let mut underline = false;
    let mut strike = false;
    let mut current = String::new();
    let mut i = 0;
    let chars: Vec<char> = html.chars().collect();

    while i < chars.len() {
        if chars[i] == '<' {
            if !current.is_empty() {
                runs.push(RunInfo {
                    text: current.clone(),
                    bold,
                    italic,
                    underline,
                    strike,
                });
                current.clear();
            }
            let end = chars[i..].iter().position(|&c| c == '>');
            let Some(end) = end else {
                current.push(chars[i]);
                i += 1;
                continue;
            };
            let tag_str: String = chars[i + 1..i + end].iter().collect();
            let is_close = tag_str.starts_with('/');
            let tag_name = if is_close { &tag_str[1..] } else { &tag_str };
            let tag_name = tag_name
                .split(|c: char| c.is_whitespace())
                .next()
                .unwrap_or("")
                .to_lowercase();
            match tag_name.as_str() {
                "b" | "strong" => bold = !is_close,
                "i" | "em" => italic = !is_close,
                "u" => underline = !is_close,
                "s" | "strike" | "del" => strike = !is_close,
                _ => {}
            }
            i += end + 1;
        } else {
            current.push(chars[i]);
            i += 1;
        }
    }
    if !current.is_empty() {
        runs.push(RunInfo {
            text: current,
            bold,
            italic,
            underline,
            strike,
        });
    }
    runs
}

// ── Helpers ─────────────────────────────────────────────────────────

fn strip_inline_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

pub fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
