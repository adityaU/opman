//! Media HTML renderers — image, pdf, link, accordion.
//!
//! All functions append raw HTML to `&mut String` for `inner_html`.

use super::html_render::{blocks_to_html, esc, md, md_inline, sf, sf_or, svg_icon};

// ── Image ───────────────────────────────────────────────────────────

/// Renders an image with optional caption.
///
/// Data fields:
/// - `url` / `src` (string, required): image URL
/// - `alt` (string, optional): alt text
/// - `caption` (string, optional): caption below image
/// - `width` (string, optional): CSS width (e.g. "100%", "300px")
/// - `height` (string, optional): CSS height
pub fn image_html(data: &serde_json::Value, out: &mut String) {
    let url = match sf_or(data, "url", "src") {
        Some(u) => u,
        None => {
            out.push_str("<div class=\"a2ui-unknown\">Image: missing url</div>");
            return;
        }
    };
    let alt = sf(data, "alt").unwrap_or_default();
    let caption = sf(data, "caption");
    let width = sf(data, "width");
    let height = sf(data, "height");

    out.push_str("<figure class=\"a2ui-image\">");

    // Build style attribute
    let mut style = String::new();
    if let Some(ref w) = width {
        style.push_str(&format!("max-width:{};", esc(w)));
    }
    if let Some(ref h) = height {
        style.push_str(&format!("max-height:{};", esc(h)));
    }

    out.push_str(&format!(
        "<img src=\"{}\" alt=\"{}\" class=\"a2ui-image-el\" loading=\"lazy\"",
        esc(&url),
        esc(&alt)
    ));
    if !style.is_empty() {
        out.push_str(&format!(" style=\"{}\"", style));
    }
    out.push_str(" />");

    if let Some(cap) = caption {
        out.push_str(&format!(
            "<figcaption class=\"a2ui-image-caption\">{}</figcaption>",
            md_inline(&cap)
        ));
    }
    out.push_str("</figure>");
}

// ── PDF ─────────────────────────────────────────────────────────────

/// Renders an embedded PDF viewer.
///
/// Data fields:
/// - `url` / `src` (string, required): PDF URL
/// - `title` (string, optional): title above the embed
/// - `height` (string, optional, default "400px"): iframe height
pub fn pdf_html(data: &serde_json::Value, out: &mut String) {
    let url = match sf_or(data, "url", "src") {
        Some(u) => u,
        None => {
            out.push_str("<div class=\"a2ui-unknown\">PDF: missing url</div>");
            return;
        }
    };
    let title = sf(data, "title");
    let height = sf(data, "height").unwrap_or_else(|| "400px".into());

    out.push_str("<div class=\"a2ui-pdf\">");
    if let Some(ref t) = title {
        out.push_str(&format!("<div class=\"a2ui-pdf-title\">{}</div>", esc(t)));
    }
    out.push_str(&format!(
        "<iframe src=\"{}\" class=\"a2ui-pdf-frame\" \
         style=\"height:{}\" loading=\"lazy\" \
         title=\"PDF document\"></iframe>",
        esc(&url),
        esc(&height)
    ));
    // Fallback link
    out.push_str(&format!(
        "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\" \
         class=\"a2ui-pdf-fallback\">Open PDF in new tab {}</a>",
        esc(&url),
        svg_icon("external-link", 12)
    ));
    out.push_str("</div>");
}

// ── Link ────────────────────────────────────────────────────────────

/// Renders a styled link that opens in a new tab.
///
/// Data fields:
/// - `url` / `href` (string, required): link URL
/// - `label` / `text` (string, optional): display text (defaults to URL)
/// - `description` (string, optional): subtitle text
pub fn link_html(data: &serde_json::Value, out: &mut String) {
    let url = match sf_or(data, "url", "href") {
        Some(u) => u,
        None => {
            out.push_str("<div class=\"a2ui-unknown\">Link: missing url</div>");
            return;
        }
    };
    let label = sf_or(data, "label", "text").unwrap_or_else(|| url.clone());
    let desc = sf(data, "description");

    out.push_str(&format!(
        "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\" class=\"a2ui-link\">",
        esc(&url)
    ));
    out.push_str(&format!(
        "<span class=\"a2ui-link-label\">{}</span>",
        esc(&label)
    ));
    if let Some(d) = desc {
        out.push_str(&format!(
            "<span class=\"a2ui-link-desc\">{}</span>",
            md_inline(&d)
        ));
    }
    // External link icon
    out.push_str(&format!(
        "<span class=\"a2ui-link-icon\">{}</span>",
        svg_icon("external-link", 12)
    ));
    out.push_str("</a>");
}

// ── Accordion ───────────────────────────────────────────────────────

/// Renders an HTML `<details>` accordion element.
///
/// Data fields:
/// - `title` / `label` (string, required): summary header
/// - `open` (bool, default false): start expanded
/// - `blocks` (array, optional): child blocks inside the accordion
/// - `content` (string, optional): plain text content (if no blocks)
pub fn accordion_html(data: &serde_json::Value, out: &mut String) {
    let title = sf_or(data, "title", "label").unwrap_or_else(|| "Details".into());
    let open = data.get("open").and_then(|v| v.as_bool()).unwrap_or(false);

    let children = data
        .get("blocks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let content = sf(data, "content");

    out.push_str("<details class=\"a2ui-accordion\"");
    if open {
        out.push_str(" open");
    }
    out.push_str(">");

    out.push_str(&format!(
        "<summary class=\"a2ui-accordion-summary\">{}</summary>",
        esc(&title)
    ));

    out.push_str("<div class=\"a2ui-accordion-body\">");
    if !children.is_empty() {
        let child_html = blocks_to_html(&children);
        out.push_str(&child_html);
    } else if let Some(c) = content {
        out.push_str(&format!("<div>{}</div>", md(&c)));
    }
    out.push_str("</div></details>");
}
