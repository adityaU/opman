//! Coding-workflow HTML renderers — diff, timeline, terminal, file-tree.

use super::html_render::{esc, sf, sf_or, svg_icon};

// ── Diff ────────────────────────────────────────────────────────────

/// Unified text diff with +/- line coloring.
pub fn diff_html(data: &serde_json::Value, out: &mut String) {
    let content = sf_or(data, "content", "text").unwrap_or_default();
    let title = sf(data, "title");
    let lang = sf(data, "language").unwrap_or_default();

    out.push_str("<div class=\"a2ui-diff\">");
    if let Some(ref t) = title {
        out.push_str(&format!("<div class=\"a2ui-diff-title\">{}</div>", esc(t)));
    }
    out.push_str(&format!(
        "<pre class=\"a2ui-diff-pre\" data-language=\"{}\"><code>",
        esc(&lang)
    ));

    for (i, line) in content.lines().enumerate() {
        let (cls, prefix) = classify_diff_line(line);
        out.push_str(&format!(
            "<span class=\"a2ui-diff-ln\">{}</span>\
             <span class=\"a2ui-diff-line {cls}\">{prefix}{}</span>\n",
            i + 1,
            esc(line.get(1..).unwrap_or(line)),
        ));
    }
    out.push_str("</code></pre></div>");
}

fn classify_diff_line(line: &str) -> (&'static str, &'static str) {
    match line.chars().next() {
        Some('+') => ("a2ui-diff-add", "+ "),
        Some('-') => ("a2ui-diff-del", "- "),
        Some('@') => ("a2ui-diff-hunk", ""),
        _ => ("a2ui-diff-ctx", "  "),
    }
}

// ── Timeline ────────────────────────────────────────────────────────

/// Vertical timeline with dated/labeled entries and status dots.
pub fn timeline_html(data: &serde_json::Value, out: &mut String) {
    let items = match data
        .get("items")
        .or_else(|| data.get("entries"))
        .and_then(|v| v.as_array())
    {
        Some(i) => i,
        None => return,
    };

    out.push_str("<div class=\"a2ui-timeline\">");
    for item in items {
        let label = item
            .get("label")
            .or_else(|| item.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let desc = item
            .get("description")
            .or_else(|| item.get("body"))
            .and_then(|v| v.as_str());
        let date = item
            .get("date")
            .or_else(|| item.get("time"))
            .and_then(|v| v.as_str());
        let status = item
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        let icon = item.get("icon").and_then(|v| v.as_str());

        out.push_str(&format!(
            "<div class=\"a2ui-tl-entry a2ui-tl-{}\">",
            esc(status)
        ));

        // Dot / icon
        out.push_str("<div class=\"a2ui-tl-dot\">");
        if let Some(ic) = icon {
            out.push_str(&esc(ic));
        } else {
            out.push_str(&timeline_dot_icon(status));
        }
        out.push_str("</div>");

        // Content
        out.push_str("<div class=\"a2ui-tl-content\">");
        if let Some(d) = date {
            out.push_str(&format!("<span class=\"a2ui-tl-date\">{}</span>", esc(d)));
        }
        out.push_str(&format!(
            "<span class=\"a2ui-tl-label\">{}</span>",
            esc(label)
        ));
        if let Some(d) = desc {
            out.push_str(&format!("<span class=\"a2ui-tl-desc\">{}</span>", esc(d)));
        }
        out.push_str("</div></div>");
    }
    out.push_str("</div>");
}

fn timeline_dot_icon(status: &str) -> String {
    match status {
        "done" | "completed" => svg_icon("check-circle", 14),
        "error" => svg_icon("x-circle", 14),
        "active" => "<span class=\"tool-pulse-dot\"></span>".into(),
        _ => "<span class=\"a2ui-tl-dot-empty\"></span>".into(),
    }
}

// ── Terminal ────────────────────────────────────────────────────────

/// Terminal output with title bar dots and basic ANSI color support.
pub fn terminal_html(data: &serde_json::Value, out: &mut String) {
    let content = sf_or(data, "content", "text").unwrap_or_default();
    let title = sf(data, "title").unwrap_or_else(|| "Terminal".into());
    let prompt = sf(data, "prompt");

    out.push_str("<div class=\"a2ui-terminal\">");
    // Title bar with dots
    out.push_str(&format!(
        "<div class=\"a2ui-term-titlebar\">\
         <span class=\"a2ui-term-dots\">\
         <span class=\"a2ui-term-dot a2ui-term-dot-r\"></span>\
         <span class=\"a2ui-term-dot a2ui-term-dot-y\"></span>\
         <span class=\"a2ui-term-dot a2ui-term-dot-g\"></span>\
         </span>\
         <span class=\"a2ui-term-title\">{}</span></div>",
        esc(&title)
    ));
    out.push_str("<pre class=\"a2ui-term-body\"><code>");
    if let Some(ref p) = prompt {
        out.push_str(&format!(
            "<span class=\"a2ui-term-prompt\">{}</span>",
            esc(p)
        ));
    }
    // Basic ANSI → HTML conversion for common codes
    out.push_str(&ansi_to_html(&content));
    out.push_str("</code></pre></div>");
}

/// Minimal ANSI escape → HTML span conversion.
fn ansi_to_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut open_spans: u32 = 0;

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            let mut code = String::new();
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
                code.push(c);
            }
            let cls = match code.as_str() {
                "0" | "" => {
                    // Reset — close all open spans
                    for _ in 0..open_spans {
                        result.push_str("</span>");
                    }
                    open_spans = 0;
                    continue;
                }
                "1" => "a2ui-ansi-bold",
                "2" => "a2ui-ansi-dim",
                "31" => "a2ui-ansi-red",
                "32" => "a2ui-ansi-green",
                "33" => "a2ui-ansi-yellow",
                "34" => "a2ui-ansi-blue",
                "35" => "a2ui-ansi-magenta",
                "36" => "a2ui-ansi-cyan",
                _ => continue,
            };
            result.push_str(&format!("<span class=\"{cls}\">"));
            open_spans += 1;
        } else {
            // Escape HTML special chars
            match ch {
                '&' => result.push_str("&amp;"),
                '<' => result.push_str("&lt;"),
                '>' => result.push_str("&gt;"),
                '"' => result.push_str("&quot;"),
                _ => result.push(ch),
            }
        }
    }
    for _ in 0..open_spans {
        result.push_str("</span>");
    }
    result
}

// ── File Tree ───────────────────────────────────────────────────────

/// Collapsible file/directory tree with file-type icons and status indicators.
pub fn file_tree_html(data: &serde_json::Value, out: &mut String) {
    let items = match data
        .get("items")
        .or_else(|| data.get("tree"))
        .and_then(|v| v.as_array())
    {
        Some(i) => i,
        None => return,
    };
    let title = sf(data, "title");

    out.push_str("<div class=\"a2ui-ftree\">");
    if let Some(ref t) = title {
        out.push_str(&format!("<div class=\"a2ui-ftree-title\">{}</div>", esc(t)));
    }
    out.push_str("<ul class=\"a2ui-ftree-list\">");
    render_tree_items(items, out, 0);
    out.push_str("</ul></div>");
}

fn render_tree_items(items: &[serde_json::Value], out: &mut String, depth: u32) {
    if depth > 10 {
        return; // prevent infinite recursion
    }
    for item in items {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("file");
        let status = item.get("status").and_then(|v| v.as_str());
        let children = item
            .get("items")
            .or_else(|| item.get("children"))
            .and_then(|v| v.as_array());

        let is_dir = kind == "dir" || kind == "directory" || children.is_some();
        let icon = if is_dir { "📁" } else { file_icon(name) };

        let status_cls = status
            .map(|s| format!(" a2ui-ftree-{}", s))
            .unwrap_or_default();

        if is_dir {
            out.push_str(&format!(
                "<li class=\"a2ui-ftree-node a2ui-ftree-dir{status_cls}\">"
            ));
            out.push_str(&format!(
                "<details open><summary class=\"a2ui-ftree-name\">\
                 <span class=\"a2ui-ftree-icon\">{icon}</span>{}</summary>",
                esc(name)
            ));
            if let Some(kids) = children {
                out.push_str("<ul class=\"a2ui-ftree-list\">");
                render_tree_items(kids, out, depth + 1);
                out.push_str("</ul>");
            }
            out.push_str("</details></li>");
        } else {
            out.push_str(&format!(
                "<li class=\"a2ui-ftree-node a2ui-ftree-file{status_cls}\">\
                 <span class=\"a2ui-ftree-name\">\
                 <span class=\"a2ui-ftree-icon\">{icon}</span>{}</span>",
                esc(name)
            ));
            if let Some(s) = status {
                out.push_str(&format!(
                    "<span class=\"a2ui-ftree-status\">{}</span>",
                    esc(s)
                ));
            }
            out.push_str("</li>");
        }
    }
}

fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "🦀",
        "js" | "jsx" | "mjs" => "🟨",
        "ts" | "tsx" => "🔷",
        "py" => "🐍",
        "css" | "scss" => "🎨",
        "html" => "🌐",
        "json" => "📋",
        "toml" | "yaml" | "yml" => "⚙️",
        "md" => "📝",
        "lock" => "🔒",
        _ => "📄",
    }
}
