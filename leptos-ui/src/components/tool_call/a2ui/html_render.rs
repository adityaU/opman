//! HTML string rendering for A2UI blocks (core).
//! Converts block JSON → raw HTML for `inner_html`, avoiding Leptos fragment accumulation.

use super::{html_render_chart, html_render_coding, html_render_content};
use super::{html_render_ext, html_render_interface, html_render_layout, html_render_media};

use super::blocks::{cell_to_string, str_field};
use crate::components::message_turn::{parse_markdown_segments, ContentSegment};

// ── Public entry ────────────────────────────────────────────────────

pub fn blocks_to_html(blocks: &[serde_json::Value]) -> String {
    let mut out = String::with_capacity(blocks.len() * 256);
    for block in blocks {
        let bt = block
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let data = block
            .get("data")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        match bt {
            "card" => card_html(&data, &mut out),
            "table" => table_html(&data, &mut out),
            "kv" => kv_html(&data, &mut out),
            "status" => status_html(&data, &mut out),
            "progress" => progress_html(&data, &mut out),
            "alert" => alert_html(&data, &mut out),
            "markdown" => markdown_html(&data, &mut out),
            "button" => html_render_ext::button_html(&data, &mut out),
            "form" => html_render_ext::form_html(&data, &mut out),
            "steps" => html_render_ext::steps_html(&data, &mut out),
            "divider" => html_render_ext::divider_html(&data, &mut out),
            "code" => html_render_ext::code_html(&data, &mut out),
            "metric" => html_render_ext::metric_html(&data, &mut out),
            "grid" => html_render_layout::grid_html(&data, &mut out),
            "flex" => html_render_layout::flex_html(&data, &mut out),
            "image" => html_render_media::image_html(&data, &mut out),
            "pdf" => html_render_media::pdf_html(&data, &mut out),
            "link" => html_render_media::link_html(&data, &mut out),
            "accordion" => html_render_media::accordion_html(&data, &mut out),
            "chart" => html_render_chart::chart_html(&data, &mut out),
            // Tier 1 content blocks
            "tabs" => html_render_content::tabs_html(&data, &mut out),
            "callout" => html_render_content::callout_html(&data, &mut out),
            "badge" => html_render_content::badge_html(&data, &mut out),
            "blockquote" => html_render_content::blockquote_html(&data, &mut out),
            "list" => html_render_content::list_html(&data, &mut out),
            "stat-group" | "stat_group" => html_render_content::stat_group_html(&data, &mut out),
            // Coding workflow blocks
            "diff" => html_render_coding::diff_html(&data, &mut out),
            "timeline" => html_render_coding::timeline_html(&data, &mut out),
            "terminal" => html_render_coding::terminal_html(&data, &mut out),
            "file-tree" | "file_tree" => html_render_coding::file_tree_html(&data, &mut out),
            // Interface blocks
            "avatar" => html_render_interface::avatar_html(&data, &mut out),
            "tag-group" | "tag_group" => html_render_interface::tag_group_html(&data, &mut out),
            "toggle" => html_render_interface::toggle_html(&data, &mut out),
            "video" => html_render_interface::video_html(&data, &mut out),
            "audio" => html_render_interface::audio_html(&data, &mut out),
            "separator" => html_render_interface::separator_html(&data, &mut out),
            _ => {
                out.push_str(&format!(
                    "<div class=\"a2ui-unknown\">Unknown block type: {}</div>",
                    esc(bt)
                ));
            }
        }
    }
    out
}

// ── Helpers (pub(super) for sibling modules) ────────────────────────

pub(super) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn sf(data: &serde_json::Value, key: &str) -> Option<String> {
    str_field(data, key)
}

pub(super) fn sf_or(data: &serde_json::Value, a: &str, b: &str) -> Option<String> {
    sf(data, a).or_else(|| sf(data, b))
}

fn f64_or(data: &serde_json::Value, a: &str, b: &str) -> Option<f64> {
    data.get(a)
        .and_then(|v| v.as_f64())
        .or_else(|| data.get(b).and_then(|v| v.as_f64()))
}

// ── SVG icons ───────────────────────────────────────────────────────

pub(super) fn svg_icon(name: &str, size: u32) -> String {
    let body = match name {
        "check-circle" => "<path d=\"M22 11.08V12a10 10 0 1 1-5.93-9.14\"/><polyline points=\"22 4 12 14.01 9 11.01\"/>",
        "alert-triangle" => "<path d=\"M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z\"/><line x1=\"12\" y1=\"9\" x2=\"12\" y2=\"13\"/><line x1=\"12\" y1=\"17\" x2=\"12.01\" y2=\"17\"/>",
        "x-circle" => "<circle cx=\"12\" cy=\"12\" r=\"10\"/><line x1=\"15\" y1=\"9\" x2=\"9\" y2=\"15\"/><line x1=\"9\" y1=\"9\" x2=\"15\" y2=\"15\"/>",
        "info" => "<circle cx=\"12\" cy=\"12\" r=\"10\"/><line x1=\"12\" y1=\"16\" x2=\"12\" y2=\"12\"/><line x1=\"12\" y1=\"8\" x2=\"12.01\" y2=\"8\"/>",
        "external-link" => "<path d=\"M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6\"/><polyline points=\"15 3 21 3 21 9\"/><line x1=\"10\" y1=\"14\" x2=\"21\" y2=\"3\"/>",
        _ => "",
    };
    format!(
        "<svg width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" \
         stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" \
         stroke-linejoin=\"round\">{}</svg>",
        size, size, body
    )
}

pub(super) fn level_icon_html(level: &str, size: u32) -> String {
    match level {
        "success" => svg_icon("check-circle", size),
        "warning" => svg_icon("alert-triangle", size),
        "error" => svg_icon("x-circle", size),
        _ => svg_icon("info", size),
    }
}

// ── Block renderers (core) ──────────────────────────────────────────

fn card_html(data: &serde_json::Value, out: &mut String) {
    let title = sf(data, "title").unwrap_or_default();
    let body = sf_or(data, "body", "content").unwrap_or_default();
    let icon = sf(data, "icon");
    out.push_str("<div class=\"a2ui-card\"><div class=\"a2ui-card-header\">");
    if let Some(i) = icon {
        out.push_str(&format!(
            "<span class=\"a2ui-card-icon\">{}</span>",
            esc(&i)
        ));
    }
    out.push_str(&format!(
        "<span class=\"a2ui-card-title\">{}</span></div>",
        esc(&title)
    ));
    if !body.is_empty() {
        out.push_str(&format!(
            "<div class=\"a2ui-card-body\">{}</div>",
            esc(&body)
        ));
    }
    out.push_str("</div>");
}

fn table_html(data: &serde_json::Value, out: &mut String) {
    let headers: Vec<String> = data
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let rows: Vec<Vec<String>> = data
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|r| {
                    r.as_array()
                        .map(|c| c.iter().map(|v| cell_to_string(v)).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();

    out.push_str("<div class=\"a2ui-table-wrap\"><table class=\"a2ui-table\">");
    if !headers.is_empty() {
        out.push_str("<thead><tr>");
        for h in &headers {
            out.push_str(&format!("<th>{}</th>", esc(h)));
        }
        out.push_str("</tr></thead>");
    }
    out.push_str("<tbody>");
    for row in &rows {
        out.push_str("<tr>");
        for cell in row {
            out.push_str(&format!("<td>{}</td>", esc(cell)));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
}

fn kv_html(data: &serde_json::Value, out: &mut String) {
    let pairs: Vec<(String, String)> = data
        .get("pairs")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|item| {
                    let k = item.get("key")?.as_str()?.to_string();
                    let v = cell_to_string(item.get("value").unwrap_or(&serde_json::Value::Null));
                    Some((k, v))
                })
                .collect()
        })
        .unwrap_or_default();

    out.push_str("<div class=\"a2ui-kv\">");
    for (k, v) in &pairs {
        out.push_str(&format!(
            "<div class=\"a2ui-kv-row\">\
             <span class=\"a2ui-kv-key\">{}</span>\
             <span class=\"a2ui-kv-val\">{}</span></div>",
            esc(k),
            esc(v)
        ));
    }
    out.push_str("</div>");
}

fn status_html(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label").unwrap_or_else(|| "Status".into());
    let level = sf(data, "level").unwrap_or_else(|| "info".into());
    let detail = sf_or(data, "detail", "message");

    out.push_str(&format!(
        "<div class=\"a2ui-status a2ui-status-{}\">{}",
        esc(&level),
        level_icon_html(&level, 14)
    ));
    out.push_str(&format!(
        "<span class=\"a2ui-status-label\">{}</span>",
        esc(&label)
    ));
    if let Some(d) = detail {
        out.push_str(&format!(
            "<span class=\"a2ui-status-detail\">{}</span>",
            esc(&d)
        ));
    }
    out.push_str("</div>");
}

fn progress_html(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label").unwrap_or_default();
    let pct = f64_or(data, "percent", "percentage")
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);

    out.push_str("<div class=\"a2ui-progress\">");
    out.push_str(&format!(
        "<div class=\"a2ui-progress-header\">\
         <span class=\"a2ui-progress-label\">{}</span>\
         <span class=\"a2ui-progress-pct\">{:.0}%</span></div>",
        esc(&label),
        pct
    ));
    out.push_str(&format!(
        "<div class=\"a2ui-progress-track\">\
         <div class=\"a2ui-progress-fill\" style=\"width: {}%\"></div>\
         </div></div>",
        pct
    ));
}

fn alert_html(data: &serde_json::Value, out: &mut String) {
    let message = sf(data, "message").unwrap_or_default();
    let level = sf(data, "level").unwrap_or_else(|| "info".into());
    let label = sf(data, "label").unwrap_or_default();

    out.push_str(&format!(
        "<div class=\"a2ui-alert a2ui-alert-{}\">{}",
        esc(&level),
        level_icon_html(&level, 14)
    ));
    if !label.is_empty() {
        out.push_str(&format!(
            "<strong class=\"a2ui-alert-label\">{}</strong>",
            esc(&label)
        ));
    }
    out.push_str(&format!("<span>{}</span></div>", esc(&message)));
}

fn markdown_html(data: &serde_json::Value, out: &mut String) {
    let content = sf(data, "content").unwrap_or_default();
    out.push_str("<div class=\"a2ui-markdown\">");
    for seg in parse_markdown_segments(&content) {
        match seg {
            ContentSegment::Html(html) => out.push_str(&format!("<div>{}</div>", html)),
            ContentSegment::FencedCode { language, code } => out.push_str(&format!(
                "<pre class=\"code-block\" data-language=\"{}\"><code>{}</code></pre>",
                esc(&language),
                esc(&code)
            )),
        }
    }
    out.push_str("</div>");
}
