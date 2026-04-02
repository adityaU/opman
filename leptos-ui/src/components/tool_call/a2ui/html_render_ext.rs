//! Extended HTML string renderers — button, form, steps, divider, code, metric.
//!
//! Companion to `html_render.rs`. All functions append raw HTML to a
//! `&mut String` for use with `inner_html`, avoiding Leptos fragment
//! accumulation in reactive closures.
//!
//! Buttons and forms emit `data-a2ui-*` attributes; event delegation in
//! `mod.rs` wires them to the `opman:a2ui-callback` custom event.

use super::html_render::{esc, level_icon_html, sf, sf_or, svg_icon};

// ── Button ──────────────────────────────────────────────────────────

pub fn button_html(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label").unwrap_or_else(|| "Click".into());
    let callback_id = sf(data, "callback_id").unwrap_or_default();
    let variant = sf(data, "variant").unwrap_or_else(|| "default".into());

    out.push_str(&format!(
        "<button class=\"a2ui-btn a2ui-btn-{}\" data-a2ui-callback=\"{}\">{}</button>",
        esc(&variant),
        esc(&callback_id),
        esc(&label),
    ));
}

// ── Form ────────────────────────────────────────────────────────────

pub fn form_html(data: &serde_json::Value, out: &mut String) {
    let callback_id = sf(data, "callback_id").unwrap_or_default();
    let submit_label = sf(data, "submit_label").unwrap_or_else(|| "Submit".into());

    let fields = data
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    out.push_str(&format!(
        "<form class=\"a2ui-form\" data-a2ui-form-callback=\"{}\">",
        esc(&callback_id)
    ));

    for f in &fields {
        let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let label = f.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let ft = f.get("type").and_then(|v| v.as_str()).unwrap_or("text");
        let placeholder = f.get("placeholder").and_then(|v| v.as_str()).unwrap_or("");
        let default = f.get("default").and_then(|v| v.as_str()).unwrap_or("");

        out.push_str("<div class=\"a2ui-form-field\">");
        out.push_str(&format!(
            "<label class=\"a2ui-form-label\">{}</label>",
            esc(label)
        ));

        if ft == "textarea" {
            out.push_str(&format!(
                "<textarea class=\"a2ui-form-input\" name=\"{}\" placeholder=\"{}\">{}</textarea>",
                esc(name),
                esc(placeholder),
                esc(default),
            ));
        } else {
            out.push_str(&format!(
                "<input class=\"a2ui-form-input\" type=\"{}\" name=\"{}\" \
                 placeholder=\"{}\" value=\"{}\" />",
                esc(ft),
                esc(name),
                esc(placeholder),
                esc(default),
            ));
        }
        out.push_str("</div>");
    }

    out.push_str(&format!(
        "<button type=\"submit\" class=\"a2ui-btn a2ui-btn-primary\">{}</button>",
        esc(&submit_label)
    ));
    out.push_str("</form>");
}

// ── Steps ───────────────────────────────────────────────────────────

pub fn steps_html(data: &serde_json::Value, out: &mut String) {
    let title = sf(data, "title");
    let items = data
        .get("items")
        .or_else(|| data.get("steps"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    out.push_str("<div class=\"a2ui-steps\">");
    if let Some(t) = title {
        out.push_str(&format!(
            "<div class=\"a2ui-steps-title\">{}</div>",
            esc(&t)
        ));
    }
    out.push_str("<ol class=\"a2ui-steps-list\">");
    for (i, item) in items.iter().enumerate() {
        let label = item.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let status = item
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");

        out.push_str(&format!(
            "<li class=\"a2ui-step a2ui-step-{}\">",
            esc(status)
        ));
        out.push_str(&format!("<span class=\"a2ui-step-num\">{}</span>", i + 1));
        step_icon_html(status, out);
        out.push_str(&format!(
            "<span class=\"a2ui-step-label\">{}</span></li>",
            esc(label)
        ));
    }
    out.push_str("</ol></div>");
}

fn step_icon_html(status: &str, out: &mut String) {
    match status {
        "done" | "completed" => {
            out.push_str(&format!(
                "<span class=\"a2ui-step-icon\">{}</span>",
                svg_icon("check-circle", 14)
            ));
        }
        "active" | "in_progress" => {
            out.push_str(
                "<span class=\"a2ui-step-icon a2ui-step-active-icon\">\
                 <span class=\"tool-pulse-dot\"></span></span>",
            );
        }
        "error" => {
            out.push_str(&format!(
                "<span class=\"a2ui-step-icon\">{}</span>",
                svg_icon("x-circle", 14)
            ));
        }
        _ => {
            out.push_str("<span class=\"a2ui-step-icon a2ui-step-pending-icon\"></span>");
        }
    }
}

// ── Divider ─────────────────────────────────────────────────────────

pub fn divider_html(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label");
    out.push_str("<div class=\"a2ui-divider\"><hr class=\"a2ui-divider-line\" />");
    if let Some(l) = label {
        out.push_str(&format!(
            "<span class=\"a2ui-divider-label\">{}</span>",
            esc(&l)
        ));
    }
    out.push_str("</div>");
}

// ── Code ────────────────────────────────────────────────────────────

pub fn code_html(data: &serde_json::Value, out: &mut String) {
    let code = sf_or(data, "code", "content").unwrap_or_default();
    let lang = sf(data, "language").unwrap_or_default();
    out.push_str(&format!(
        "<pre class=\"code-block\" data-language=\"{}\"><code>{}</code></pre>",
        esc(&lang),
        esc(&code)
    ));
}

// ── Metric ──────────────────────────────────────────────────────────

pub fn metric_html(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label").unwrap_or_default();
    let value = sf(data, "value").unwrap_or_else(|| {
        data.get("value")
            .map(|v| match v {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => String::new(),
                other => serde_json::to_string(other).unwrap_or_default(),
            })
            .unwrap_or_default()
    });
    let trend = sf(data, "trend");
    let description = sf(data, "description");

    out.push_str("<div class=\"a2ui-metric\">");
    out.push_str(&format!(
        "<span class=\"a2ui-metric-label\">{}</span>",
        esc(&label)
    ));
    out.push_str(&format!(
        "<span class=\"a2ui-metric-value\">{}</span>",
        esc(&value)
    ));

    if let Some(ref t) = trend {
        let cls = match t.as_str() {
            "up" => "a2ui-metric-trend-up",
            "down" => "a2ui-metric-trend-down",
            _ => "a2ui-metric-trend-flat",
        };
        let arrow = match t.as_str() {
            "up" => "↑",
            "down" => "↓",
            _ => "→",
        };
        out.push_str(&format!(
            "<span class=\"a2ui-metric-trend {}\">{}</span>",
            cls, arrow
        ));
    }

    if let Some(d) = description {
        out.push_str(&format!(
            "<span class=\"a2ui-metric-desc\">{}</span>",
            esc(&d)
        ));
    }
    out.push_str("</div>");
}
