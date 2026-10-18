//! A small HTML to readable-text converter: enough for docs and articles, with headings,
//! links, lists and code kept as Markdown. It is not a full parser.

/// Tags whose whole content is dropped.
const SKIPPED: [&str; 7] = [
    "script", "style", "head", "noscript", "svg", "template", "iframe",
];
/// Tags that start a new block of text.
const BLOCKS: [&str; 16] = [
    "p",
    "div",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "main",
    "ul",
    "ol",
    "table",
    "tr",
    "blockquote",
    "br",
    "hr",
    "form",
];

pub fn to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 3);
    let mut rest = html;
    let mut in_pre = false;
    // Where each open link's `[` sits in `out`, and its target.
    let mut links: Vec<(usize, String)> = Vec::new();
    while let Some(open) = rest.find('<') {
        push_text(&mut out, &rest[..open], in_pre);
        rest = &rest[open..];
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->").map_or("", |(_, r)| r);
            continue;
        }
        let Some(close) = rest.find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];
        let (name, closing) = tag_name(tag);
        if !closing && SKIPPED.contains(&name.as_str()) && !tag.ends_with('/') {
            rest = skip_past_close(rest, &name);
            continue;
        }
        match (name.as_str(), closing) {
            ("pre", closing) => {
                in_pre = !closing;
                out.push_str("\n```\n");
            }
            ("a", false) => {
                links.push((out.len(), attribute(tag, "href").unwrap_or_default()));
                out.push('[');
            }
            ("a", true) => {
                let (at, href) = links.pop().unwrap_or((out.len(), String::new()));
                if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
                    if out.is_char_boundary(at) && out[at..].starts_with('[') {
                        out.remove(at);
                    }
                } else {
                    out.push_str(&format!("]({href})"));
                }
            }
            ("img", false) => {
                if let Some(alt) = attribute(tag, "alt").filter(|a| !a.is_empty()) {
                    out.push_str(&format!("[image: {alt}]"));
                }
            }
            ("li", false) => out.push_str("\n- "),
            ("code", _) | ("tt", _) if !in_pre => out.push('`'),
            ("strong" | "b", _) => out.push_str("**"),
            ("em" | "i", _) => out.push('_'),
            (h, false) if is_heading(h) => {
                let level = h[1..].parse::<usize>().unwrap_or(1);
                out.push_str(&format!("\n\n{} ", "#".repeat(level)));
            }
            (h, true) if is_heading(h) => out.push_str("\n\n"),
            (b, _) if BLOCKS.contains(&b) => out.push('\n'),
            _ => {}
        }
    }
    push_text(&mut out, rest, in_pre);
    tidy(&out)
}

fn is_heading(name: &str) -> bool {
    name.len() == 2 && name.starts_with('h') && name.as_bytes()[1].is_ascii_digit()
}

/// The lower-case tag name and whether this is a closing tag.
fn tag_name(tag: &str) -> (String, bool) {
    let (closing, body) = match tag.strip_prefix('/') {
        Some(body) => (true, body),
        None => (false, tag),
    };
    let name: String = body
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (name, closing)
}

fn skip_past_close<'a>(rest: &'a str, name: &str) -> &'a str {
    let needle = format!("</{name}");
    let lower = rest.to_ascii_lowercase();
    match lower.find(&needle) {
        Some(at) => rest[at..].split_once('>').map_or("", |(_, r)| r),
        None => "",
    }
}

fn attribute(tag: &str, wanted: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(wanted) {
        let start = from + at;
        from = start + wanted.len();
        let before_ok = lower[..start].ends_with(char::is_whitespace);
        let value = tag[from..].trim_start().strip_prefix('=')?.trim_start();
        if !before_ok {
            continue;
        }
        let (quote, value) = match value.chars().next()? {
            q @ ('"' | '\'') => (Some(q), &value[1..]),
            _ => (None, value),
        };
        let end = match quote {
            Some(q) => value.find(q)?,
            None => value.find(char::is_whitespace).unwrap_or(value.len()),
        };
        return Some(decode(&value[..end]));
    }
    None
}

fn push_text(out: &mut String, text: &str, keep_space: bool) {
    let text = decode(text);
    if keep_space {
        out.push_str(&text);
        return;
    }
    for character in text.chars() {
        if character.is_whitespace() {
            if !out.is_empty() && !out.ends_with([' ', '\n']) {
                out.push(' ');
            }
            continue;
        }
        out.push(character);
    }
}

/// Basic named and numeric entities.
fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let entity = rest
            .find(';')
            .filter(|&end| end <= 10)
            .map(|end| &rest[1..end]);
        match entity.and_then(entity_char) {
            Some(c) => {
                out.push(c);
                rest = &rest[rest.find(';').map_or(1, |end| end + 1)..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity_char(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "mdash" => Some('\u{2014}'),
        "ndash" => Some('\u{2013}'),
        "hellip" => Some('\u{2026}'),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// Trim trailing spaces and squeeze runs of blank lines to one.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim().to_string()
}
