//! Layout HTML renderers — grid + flex containers.
//!
//! These wrap child blocks in CSS grid / flexbox containers.
//! Child blocks are recursively rendered via `blocks_to_html`.

use super::html_render::{blocks_to_html, esc, sf};

// ── Grid ────────────────────────────────────────────────────────────

/// Renders a CSS grid container wrapping child blocks.
///
/// Data fields:
/// - `columns` (number, default 2): column count
/// - `gap` (string, optional): CSS gap value (e.g. "8px")
/// - `min_col_width` (string, optional): min column width for auto-fit
/// - `blocks` (array): child blocks to render inside the grid
pub fn grid_html(data: &serde_json::Value, out: &mut String) {
    let cols = data
        .get("columns")
        .and_then(|v| v.as_u64())
        .unwrap_or(2)
        .min(12) as u32;
    let gap = sf(data, "gap");
    let min_w = sf(data, "min_col_width");

    let children = data
        .get("blocks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Build inline style
    let mut style = String::with_capacity(128);
    if let Some(ref mw) = min_w {
        // Auto-fit responsive grid
        style.push_str(&format!(
            "grid-template-columns:repeat(auto-fit,minmax({},1fr));",
            esc(mw)
        ));
    } else {
        style.push_str(&format!("grid-template-columns:repeat({},1fr);", cols));
    }
    if let Some(ref g) = gap {
        style.push_str(&format!("gap:{};", esc(g)));
    }

    out.push_str(&format!("<div class=\"a2ui-grid\" style=\"{}\">", style));

    // Render each child block wrapped in a grid cell
    for child in &children {
        out.push_str("<div class=\"a2ui-grid-cell\">");
        // Single-block render: wrap in array for blocks_to_html
        let child_html = blocks_to_html(&[child.clone()]);
        out.push_str(&child_html);
        out.push_str("</div>");
    }

    out.push_str("</div>");
}

// ── Flex ─────────────────────────────────────────────────────────────

/// Renders a CSS flexbox container wrapping child blocks.
///
/// Data fields:
/// - `direction` (string, default "row"): "row" | "column"
/// - `gap` (string, optional): CSS gap value
/// - `align` (string, optional): align-items value
/// - `justify` (string, optional): justify-content value
/// - `wrap` (bool, default false): flex-wrap: wrap
/// - `blocks` (array): child blocks to render inside the flex container
pub fn flex_html(data: &serde_json::Value, out: &mut String) {
    let dir = sf(data, "direction").unwrap_or_else(|| "row".into());
    let gap = sf(data, "gap");
    let align = sf(data, "align");
    let justify = sf(data, "justify");
    let wrap = data.get("wrap").and_then(|v| v.as_bool()).unwrap_or(false);

    let children = data
        .get("blocks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut style = String::with_capacity(128);
    style.push_str(&format!("flex-direction:{};", esc(&dir)));
    if let Some(ref g) = gap {
        style.push_str(&format!("gap:{};", esc(g)));
    }
    if let Some(ref a) = align {
        style.push_str(&format!("align-items:{};", esc(a)));
    }
    if let Some(ref j) = justify {
        style.push_str(&format!("justify-content:{};", esc(j)));
    }
    if wrap {
        style.push_str("flex-wrap:wrap;");
    }

    out.push_str(&format!("<div class=\"a2ui-flex\" style=\"{}\">", style));

    for child in &children {
        out.push_str("<div class=\"a2ui-flex-item\">");
        let child_html = blocks_to_html(&[child.clone()]);
        out.push_str(&child_html);
        out.push_str("</div>");
    }

    out.push_str("</div>");
}
