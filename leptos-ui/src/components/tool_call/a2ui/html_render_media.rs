//! Media HTML renderers — image, pdf, link, accordion, mermaid.
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

// ── Mermaid ─────────────────────────────────────────────────────────

/// Mermaid diagram block. Renders the source into a `<pre class="mermaid">`
/// element which the global mermaid.js (loaded via CDN in index.html) will
/// process into an SVG after mount via `mermaid.run()` in `wire_a2ui_events`.
///
/// Accepts: `{ content/text/code: "graph TD; A-->B", title?: "..." }`
pub fn mermaid_html(data: &serde_json::Value, out: &mut String) {
    let content = sf_or(data, "content", "text")
        .or_else(|| sf(data, "code"))
        .unwrap_or_default();
    if content.is_empty() {
        out.push_str("<div class=\"a2ui-unknown\">Mermaid: empty diagram source</div>");
        return;
    }
    let title = sf(data, "title");

    out.push_str("<div class=\"a2ui-mermaid\">");

    // Toolbar: zoom-in, zoom-out, reset — wired via delegated handler in mod.rs
    out.push_str(concat!(
        "<div class=\"a2ui-mermaid-toolbar\">",
        "<button class=\"a2ui-mermaid-zoom-btn\" data-a2ui-mermaid-zoom=\"out\" title=\"Zoom out\">",
        "<svg width=\"14\" height=\"14\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\">",
        "<circle cx=\"11\" cy=\"11\" r=\"8\"/><line x1=\"21\" y1=\"21\" x2=\"16.65\" y2=\"16.65\"/>",
        "<line x1=\"8\" y1=\"11\" x2=\"14\" y2=\"11\"/></svg></button>",
        "<button class=\"a2ui-mermaid-zoom-btn\" data-a2ui-mermaid-zoom=\"reset\" title=\"Reset zoom\">",
        "<svg width=\"14\" height=\"14\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\">",
        "<path d=\"M3 12a9 9 0 1 0 9-9 4.5 4.5 0 0 0-4.5 4.5\"/><path d=\"M3 3v4.5h4.5\"/></svg></button>",
        "<button class=\"a2ui-mermaid-zoom-btn\" data-a2ui-mermaid-zoom=\"in\" title=\"Zoom in\">",
        "<svg width=\"14\" height=\"14\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\">",
        "<circle cx=\"11\" cy=\"11\" r=\"8\"/><line x1=\"21\" y1=\"21\" x2=\"16.65\" y2=\"16.65\"/>",
        "<line x1=\"11\" y1=\"8\" x2=\"11\" y2=\"14\"/><line x1=\"8\" y1=\"11\" x2=\"14\" y2=\"11\"/></svg></button>",
        "</div>",
    ));

    if let Some(ref t) = title {
        out.push_str(&format!(
            "<div class=\"a2ui-mermaid-title\">{}</div>",
            esc(t)
        ));
    }
    // Viewport wraps the diagram for CSS transform-based zoom
    out.push_str("<div class=\"a2ui-mermaid-viewport\">");
    // mermaid.js picks up <pre class="mermaid"> and replaces content with SVG
    out.push_str(&format!("<pre class=\"mermaid\">{}</pre>", esc(&content)));
    out.push_str("</div></div>");
}
