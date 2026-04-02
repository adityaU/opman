//! Chart shared helpers + pie/donut renderers.
//!
//! Split from `html_render_chart.rs` to stay under 300 lines per file.

use super::html_render::esc;

pub const W: f64 = 400.0;
pub const H: f64 = 200.0;
pub const PAD: f64 = 30.0;

const COLORS: &[&str] = &[
    "#6366f1", "#22d3ee", "#f59e0b", "#ef4444", "#10b981", "#8b5cf6", "#f97316", "#ec4899",
    "#14b8a6", "#64748b",
];

fn color(i: usize) -> &'static str {
    COLORS[i % COLORS.len()]
}

// ── Data extraction ─────────────────────────────────────────────────

pub fn extract_datasets(data: &serde_json::Value) -> Vec<(String, Vec<f64>, String)> {
    let Some(ds) = data.get("datasets").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    ds.iter()
        .enumerate()
        .map(|(i, d)| {
            let label = d
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let vals: Vec<f64> = d
                .get("data")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
                .unwrap_or_default();
            let c = d
                .get("color")
                .and_then(|v| v.as_str())
                .unwrap_or(color(i))
                .to_string();
            (label, vals, c)
        })
        .collect()
}

pub fn extract_labels(data: &serde_json::Value) -> Vec<String> {
    data.get("labels")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

pub fn extract_pie_values(data: &serde_json::Value) -> Vec<(String, f64, String)> {
    let Some(vs) = data.get("values").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    vs.iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let label = v.get("label").and_then(|l| l.as_str())?.to_string();
            let val = v.get("value").and_then(|n| n.as_f64())?;
            let c = v
                .get("color")
                .and_then(|c| c.as_str())
                .unwrap_or(color(i))
                .to_string();
            Some((label, val, c))
        })
        .collect()
}

// ── Scale / layout helpers ──────────────────────────────────────────

pub fn data_range(datasets: &[(String, Vec<f64>, String)]) -> (f64, f64) {
    let (mut min, mut max) = (f64::MAX, f64::MIN);
    for (_, vals, _) in datasets {
        for &v in vals {
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
    }
    if min == max {
        max = min + 1.0;
    }
    if min > 0.0 {
        min = 0.0;
    }
    (min, max)
}

pub fn scale_y(v: f64, min: f64, max: f64) -> f64 {
    let plot_h = H - PAD * 2.0;
    PAD + plot_h - ((v - min) / (max - min)) * plot_h
}

pub fn scale_x(i: usize, count: usize) -> f64 {
    let plot_w = W - PAD * 2.0;
    if count <= 1 {
        return PAD + plot_w / 2.0;
    }
    PAD + (i as f64 / (count - 1) as f64) * plot_w
}

pub fn svg_open(out: &mut String) {
    out.push_str(&format!(
        "<svg viewBox=\"0 0 {} {}\" class=\"a2ui-chart-svg\" \
         preserveAspectRatio=\"xMidYMid meet\">",
        W, H
    ));
}

pub fn render_x_labels(labels: &[String], out: &mut String) {
    let n = labels.len();
    for (i, label) in labels.iter().enumerate() {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" class=\"a2ui-chart-label\">{}</text>",
            scale_x(i, n),
            H - 4.0,
            esc(label)
        ));
    }
}

pub fn render_grid_lines(min: f64, max: f64, out: &mut String) {
    for i in 0..=4u32 {
        let v = min + (max - min) * (i as f64 / 4.0);
        let y = scale_y(v, min, max);
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"a2ui-chart-grid\"/>",
            PAD,
            y,
            W - PAD,
            y
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" class=\"a2ui-chart-y-label\">{}</text>",
            PAD - 4.0,
            y + 3.0,
            format_num(v)
        ));
    }
}

fn format_num(v: f64) -> String {
    if v.abs() >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v.abs() >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else if v.fract() == 0.0 {
        format!("{:.0}", v)
    } else {
        format!("{:.1}", v)
    }
}

// ── Pie / Donut chart ───────────────────────────────────────────────

pub fn pie_chart(data: &serde_json::Value, out: &mut String, donut: bool) {
    let values = extract_pie_values(data);
    if values.is_empty() {
        return;
    }
    let total: f64 = values.iter().map(|(_, v, _)| v).sum();
    if total <= 0.0 {
        return;
    }

    let cx = W / 2.0;
    let cy = H / 2.0;
    let r = (H / 2.0 - PAD).min(W / 2.0 - PAD);
    let inner_r = if donut { r * 0.55 } else { 0.0 };

    svg_open(out);
    let mut angle = -std::f64::consts::FRAC_PI_2;
    for (_, val, c) in &values {
        let sweep = (val / total) * std::f64::consts::TAU;
        let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
        let (x1, y1) = (cx + r * angle.cos(), cy + r * angle.sin());
        let (x2, y2) = (
            cx + r * (angle + sweep).cos(),
            cy + r * (angle + sweep).sin(),
        );

        if donut {
            let (ix1, iy1) = (cx + inner_r * angle.cos(), cy + inner_r * angle.sin());
            let (ix2, iy2) = (
                cx + inner_r * (angle + sweep).cos(),
                cy + inner_r * (angle + sweep).sin(),
            );
            out.push_str(&format!(
                "<path d=\"M{},{} A{},{} 0 {} 1 {},{} L{},{} A{},{} 0 {} 0 {},{} Z\" fill=\"{}\"/>",
                x1, y1, r, r, large, x2, y2, ix2, iy2, inner_r, inner_r, large, ix1, iy1, c
            ));
        } else {
            out.push_str(&format!(
                "<path d=\"M{},{} L{},{} A{},{} 0 {} 1 {},{} Z\" fill=\"{}\"/>",
                cx, cy, x1, y1, r, r, large, x2, y2, c
            ));
        }
        angle += sweep;
    }
    out.push_str("</svg>");
}

// ── Legend ───────────────────────────────────────────────────────────

pub fn render_legend(data: &serde_json::Value, chart_type: &str, out: &mut String) {
    out.push_str("<div class=\"a2ui-chart-legend\">");
    match chart_type {
        "pie" | "donut" => {
            for (label, _, c) in extract_pie_values(data) {
                legend_item(&label, &c, out);
            }
        }
        _ => {
            for (label, _, c) in extract_datasets(data) {
                if !label.is_empty() {
                    legend_item(&label, &c, out);
                }
            }
        }
    }
    out.push_str("</div>");
}

fn legend_item(label: &str, clr: &str, out: &mut String) {
    out.push_str(&format!(
        "<span class=\"a2ui-chart-legend-item\">\
         <span class=\"a2ui-chart-legend-dot\" style=\"background:{}\"></span>{}</span>",
        esc(clr),
        esc(label)
    ));
}
