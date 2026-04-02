//! Chart HTML renderer — pure SVG charts (line, bar, area).
//!
//! Companion: `html_render_chart_ext.rs` handles pie/donut + shared helpers.

use super::html_render::{esc, sf};
use super::html_render_chart_ext::{
    data_range, extract_datasets, extract_labels, render_grid_lines, render_legend,
    render_x_labels, scale_x, scale_y, svg_open,
};

// ── Public entry ────────────────────────────────────────────────────

/// Renders a chart block (dispatches to sub-type).
pub fn chart_html(data: &serde_json::Value, out: &mut String) {
    let chart_type = sf(data, "chart_type").unwrap_or_else(|| "bar".into());
    let title = sf(data, "title");

    out.push_str("<div class=\"a2ui-chart\">");
    if let Some(ref t) = title {
        out.push_str(&format!("<div class=\"a2ui-chart-title\">{}</div>", esc(t)));
    }

    match chart_type.as_str() {
        "line" => line_chart(data, out),
        "area" => area_chart(data, out),
        "bar" => bar_chart(data, out),
        "pie" => super::html_render_chart_ext::pie_chart(data, out, false),
        "donut" => super::html_render_chart_ext::pie_chart(data, out, true),
        _ => out.push_str(&format!(
            "<div class=\"a2ui-unknown\">Unknown chart type: {}</div>",
            esc(&chart_type)
        )),
    }

    render_legend(data, &chart_type, out);
    out.push_str("</div>");
}

// ── Line chart ──────────────────────────────────────────────────────

fn line_chart(data: &serde_json::Value, out: &mut String) {
    let labels = extract_labels(data);
    let datasets = extract_datasets(data);
    if datasets.is_empty() {
        return;
    }
    let (min, max) = data_range(&datasets);
    let n = labels.len();

    svg_open(out);
    render_grid_lines(min, max, out);
    render_x_labels(&labels, out);

    for (_, vals, c) in &datasets {
        let points: String = vals
            .iter()
            .enumerate()
            .map(|(i, &v)| format!("{},{}", scale_x(i, n), scale_y(v, min, max)))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" \
             stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
            points, c
        ));
        for (i, &v) in vals.iter().enumerate() {
            out.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"{}\"/>",
                scale_x(i, n),
                scale_y(v, min, max),
                c
            ));
        }
    }
    out.push_str("</svg>");
}

// ── Area chart ──────────────────────────────────────────────────────

fn area_chart(data: &serde_json::Value, out: &mut String) {
    let labels = extract_labels(data);
    let datasets = extract_datasets(data);
    if datasets.is_empty() {
        return;
    }
    let (min, max) = data_range(&datasets);
    let n = labels.len();
    let base_y = scale_y(min, min, max);

    svg_open(out);
    render_grid_lines(min, max, out);
    render_x_labels(&labels, out);

    for (_, vals, c) in &datasets {
        let mut path = format!("M{},{}", scale_x(0, n), base_y);
        for (i, &v) in vals.iter().enumerate() {
            path.push_str(&format!(" L{},{}", scale_x(i, n), scale_y(v, min, max)));
        }
        path.push_str(&format!(
            " L{},{} Z",
            scale_x(n.saturating_sub(1), n),
            base_y
        ));
        out.push_str(&format!(
            "<path d=\"{}\" fill=\"{}\" fill-opacity=\"0.15\" stroke=\"{}\" stroke-width=\"1.5\"/>",
            path, c, c
        ));
    }
    out.push_str("</svg>");
}

// ── Bar chart ───────────────────────────────────────────────────────

fn bar_chart(data: &serde_json::Value, out: &mut String) {
    let labels = extract_labels(data);
    let datasets = extract_datasets(data);
    if datasets.is_empty() {
        return;
    }
    let (min, max) = data_range(&datasets);
    let n = labels.len();
    let ds_count = datasets.len();
    let plot_w = super::html_render_chart_ext::W - super::html_render_chart_ext::PAD * 2.0;
    let group_w = if n > 0 { plot_w / n as f64 } else { plot_w };
    let bar_w = (group_w * 0.7) / ds_count as f64;
    let base_y = scale_y(0.0_f64.max(min), min, max);

    svg_open(out);
    render_grid_lines(min, max, out);
    render_x_labels(&labels, out);

    for (di, (_, vals, c)) in datasets.iter().enumerate() {
        for (i, &v) in vals.iter().enumerate() {
            let group_x = super::html_render_chart_ext::PAD + i as f64 * group_w;
            let offset = (group_w - bar_w * ds_count as f64) / 2.0;
            let x = group_x + offset + di as f64 * bar_w;
            let y = scale_y(v, min, max);
            let h = (base_y - y).abs();
            let top = y.min(base_y);
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
                 rx=\"2\" fill=\"{}\" fill-opacity=\"0.85\"/>",
                x,
                top,
                bar_w * 0.9,
                h,
                c
            ));
        }
    }
    out.push_str("</svg>");
}
