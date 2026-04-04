//! Content HTML renderers — tabs, callout, badge, blockquote, list, stat-group.

use super::html_render::{blocks_to_html, esc, md, md_inline, sf, sf_or, svg_icon};

// ── Tabs ────────────────────────────────────────────────────────────

/// Tabbed content panels. CSS-only switching via hidden radio + :checked.
pub fn tabs_html(data: &serde_json::Value, out: &mut String) {
    let tabs = match data.get("tabs").and_then(|v| v.as_array()) {
        Some(t) if !t.is_empty() => t,
        _ => {
            out.push_str("<div class=\"a2ui-unknown\">Tabs: missing tabs array</div>");
            return;
        }
    };
    let active = data.get("active").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    // Unique ID prefix for radio inputs (avoid collisions across renders)
    let uid = simple_uid();

    out.push_str("<div class=\"a2ui-tabs\">");

    // Tab header row
    out.push_str("<div class=\"a2ui-tabs-header\" role=\"tablist\">");
    for (i, tab) in tabs.iter().enumerate() {
        let label = tab.get("label").and_then(|v| v.as_str()).unwrap_or("Tab");
        let checked = if i == active { " checked" } else { "" };
        let id = format!("{}-{}", uid, i);
        out.push_str(&format!(
            "<input type=\"radio\" name=\"{uid}\" id=\"{id}\" \
             class=\"a2ui-tabs-radio\"{checked} />\
             <label for=\"{id}\" class=\"a2ui-tabs-label\" role=\"tab\">{}</label>",
            esc(label),
        ));
    }
    out.push_str("</div>");

    // Tab panels — no static active class; visibility is driven entirely
    // by the CSS :has() selectors matching the checked radio above.
    out.push_str("<div class=\"a2ui-tabs-panels\">");
    for (i, tab) in tabs.iter().enumerate() {
        let children = tab
            .get("blocks")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        out.push_str(&format!(
            "<div class=\"a2ui-tab-panel\" data-tab-index=\"{}\" role=\"tabpanel\">",
            i
        ));
        if !children.is_empty() {
            out.push_str(&blocks_to_html(&children));
        }
        out.push_str("</div>");
    }
    out.push_str("</div></div>");
}

/// Simple monotonic counter for unique IDs within a render pass.
fn simple_uid() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CTR: AtomicU32 = AtomicU32::new(0);
    format!("a2t{}", CTR.fetch_add(1, Ordering::Relaxed))
}

// ── Callout ─────────────────────────────────────────────────────────

/// Rich callout box (tip/note/warning/danger/info).
pub fn callout_html(data: &serde_json::Value, out: &mut String) {
    let variant = sf(data, "variant").unwrap_or_else(|| "info".into());
    let title = sf(data, "title").unwrap_or_else(|| variant_title(&variant));
    let body = sf_or(data, "body", "content");
    let children = data
        .get("blocks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let icon = callout_icon(&variant);

    out.push_str(&format!(
        "<div class=\"a2ui-callout a2ui-callout-{}\">",
        esc(&variant)
    ));
    out.push_str(&format!(
        "<div class=\"a2ui-callout-header\">{}<span class=\"a2ui-callout-title\">{}</span></div>",
        icon,
        esc(&title)
    ));

    if !children.is_empty() {
        out.push_str("<div class=\"a2ui-callout-body\">");
        out.push_str(&blocks_to_html(&children));
        out.push_str("</div>");
    } else if let Some(b) = body {
        out.push_str(&format!(
            "<div class=\"a2ui-callout-body\">{}</div>",
            md(&b)
        ));
    }
    out.push_str("</div>");
}

fn variant_title(v: &str) -> String {
    match v {
        "tip" => "Tip".into(),
        "note" => "Note".into(),
        "warning" => "Warning".into(),
        "danger" => "Danger".into(),
        _ => "Info".into(),
    }
}

fn callout_icon(v: &str) -> String {
    match v {
        "tip" => svg_icon("check-circle", 16),
        "warning" => svg_icon("alert-triangle", 16),
        "danger" => svg_icon("x-circle", 16),
        _ => svg_icon("info", 16),
    }
}

// ── Badge ───────────────────────────────────────────────────────────

/// Inline badge(s)/tags. Supports single or `badges[]` array.
pub fn badge_html(data: &serde_json::Value, out: &mut String) {
    if let Some(badges) = data.get("badges").and_then(|v| v.as_array()) {
        out.push_str("<span class=\"a2ui-badge-group\">");
        for b in badges {
            render_single_badge(b, out);
        }
        out.push_str("</span>");
        return;
    }
    // Single badge mode
    render_single_badge(data, out);
}

fn render_single_badge(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label").unwrap_or_else(|| "—".into());
    let variant = sf(data, "variant").unwrap_or_else(|| "neutral".into());
    out.push_str(&format!(
        "<span class=\"a2ui-badge a2ui-badge-{}\">{}</span>",
        esc(&variant),
        esc(&label)
    ));
}

// ── Blockquote ──────────────────────────────────────────────────────

/// Styled quotation with optional attribution.
pub fn blockquote_html(data: &serde_json::Value, out: &mut String) {
    let content = sf_or(data, "content", "text").unwrap_or_default();
    let attr = sf(data, "attribution")
        .or_else(|| sf(data, "author"))
        .or_else(|| sf(data, "cite"));

    out.push_str("<blockquote class=\"a2ui-blockquote\">");
    out.push_str(&md(&content));
    if let Some(a) = attr {
        out.push_str(&format!(
            "<footer class=\"a2ui-blockquote-attr\">&mdash; {}</footer>",
            esc(&a)
        ));
    }
    out.push_str("</blockquote>");
}

// ── List ────────────────────────────────────────────────────────────

/// Structured list with optional per-item icons, descriptions, nested sublists.
pub fn list_html(data: &serde_json::Value, out: &mut String) {
    let items = match data.get("items").and_then(|v| v.as_array()) {
        Some(i) => i,
        None => return,
    };
    let ordered = data
        .get("ordered")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let tag = if ordered { "ol" } else { "ul" };
    out.push_str(&format!("<{} class=\"a2ui-list\">", tag));
    render_list_items(items, out, ordered);
    out.push_str(&format!("</{}>", tag));
}

fn render_list_items(items: &[serde_json::Value], out: &mut String, ordered: bool) {
    for item in items {
        let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let icon = item.get("icon").and_then(|v| v.as_str());
        let desc = item.get("description").and_then(|v| v.as_str());
        let children = item.get("items").and_then(|v| v.as_array());

        out.push_str("<li class=\"a2ui-list-item\">");
        if let Some(ic) = icon {
            out.push_str(&format!(
                "<span class=\"a2ui-list-icon\">{}</span>",
                esc(ic)
            ));
        }
        out.push_str(&format!(
            "<span class=\"a2ui-list-text\">{}</span>",
            md_inline(text)
        ));
        if let Some(d) = desc {
            out.push_str(&format!(
                "<span class=\"a2ui-list-desc\">{}</span>",
                md_inline(d)
            ));
        }
        if let Some(sub) = children {
            let tag = if ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{} class=\"a2ui-list a2ui-list-nested\">", tag));
            render_list_items(sub, out, ordered);
            out.push_str(&format!("</{}>", tag));
        }
        out.push_str("</li>");
    }
}

// ── Stat Group ──────────────────────────────────────────────────────

/// Compact row of 2-6 metrics with trend arrows.
pub fn stat_group_html(data: &serde_json::Value, out: &mut String) {
    let stats = match data.get("stats").and_then(|v| v.as_array()) {
        Some(s) => s,
        None => return,
    };

    let count = stats.len().min(6);
    out.push_str(&format!(
        "<div class=\"a2ui-stat-group\" style=\"grid-template-columns:repeat({},1fr)\">",
        count
    ));

    for stat in stats.iter().take(count) {
        let label = stat.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let value = stat
            .get("value")
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                other => serde_json::to_string(other).unwrap_or_default(),
            })
            .unwrap_or_default();
        let trend = stat.get("trend").and_then(|v| v.as_str());
        let desc = stat.get("description").and_then(|v| v.as_str());

        out.push_str("<div class=\"a2ui-stat\">");
        out.push_str(&format!(
            "<span class=\"a2ui-stat-label\">{}</span>",
            esc(label)
        ));
        out.push_str("<div class=\"a2ui-stat-row\">");
        out.push_str(&format!(
            "<span class=\"a2ui-stat-value\">{}</span>",
            esc(&value)
        ));
        if let Some(t) = trend {
            let (cls, arrow) = match t {
                "up" => ("a2ui-trend-up", "↑"),
                "down" => ("a2ui-trend-down", "↓"),
                _ => ("a2ui-trend-flat", "→"),
            };
            out.push_str(&format!("<span class=\"{cls}\">{arrow}</span>"));
        }
        out.push_str("</div>");
        if let Some(d) = desc {
            out.push_str(&format!(
                "<span class=\"a2ui-stat-desc\">{}</span>",
                md_inline(d)
            ));
        }
        out.push_str("</div>");
    }
    out.push_str("</div>");
}
