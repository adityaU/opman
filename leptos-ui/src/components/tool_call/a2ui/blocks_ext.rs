//! Extended A2UI block renderers — steps, divider, code, metric.

use crate::components::icons::*;
use leptos::prelude::*;

use super::blocks::{cell_to_string, str_field};

// ── Helpers (re-used from blocks.rs privately) ──────────────────

fn str_field_or(data: &serde_json::Value, primary: &str, fallback: &str) -> Option<String> {
    str_field(data, primary).or_else(|| str_field(data, fallback))
}

// ── Steps ────────────────────────────────────────────────────────

pub fn render_steps(data: serde_json::Value) -> impl IntoView {
    let title = str_field(&data, "title");
    let steps: Vec<(String, String)> = data
        .get("items")
        .or_else(|| data.get("steps"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|item| {
                    let label = item
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let status = item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("pending")
                        .to_string();
                    (label, status)
                })
                .collect()
        })
        .unwrap_or_default();

    view! {
        <div class="a2ui-steps">
            {title.map(|t| view! { <div class="a2ui-steps-title">{t}</div> })}
            <ol class="a2ui-steps-list">
                {steps.into_iter().enumerate().map(|(i, (label, status))| {
                    let cls = format!("a2ui-step a2ui-step-{}", status);
                    view! {
                        <li class=cls>
                            <span class="a2ui-step-num">{i + 1}</span>
                            {step_icon(&status)}
                            <span class="a2ui-step-label">{label}</span>
                        </li>
                    }
                }).collect_view()}
            </ol>
        </div>
    }
}

fn step_icon(status: &str) -> impl IntoView {
    match status {
        "done" | "completed" => view! {
            <span class="a2ui-step-icon"><IconCheckCircle2 size=14 /></span>
        }
        .into_any(),
        "active" | "in_progress" => view! {
            <span class="a2ui-step-icon a2ui-step-active-icon"><span class="tool-pulse-dot" /></span>
        }
        .into_any(),
        "error" => view! {
            <span class="a2ui-step-icon"><IconXCircle size=14 /></span>
        }
        .into_any(),
        _ => view! {
            <span class="a2ui-step-icon a2ui-step-pending-icon">{""}</span>
        }
        .into_any(),
    }
}

// ── Divider ──────────────────────────────────────────────────────

pub fn render_divider(data: serde_json::Value) -> impl IntoView {
    let label = str_field(&data, "label");

    view! {
        <div class="a2ui-divider">
            <hr class="a2ui-divider-line" />
            {label.map(|l| view! { <span class="a2ui-divider-label">{l}</span> })}
        </div>
    }
}

// ── Code ─────────────────────────────────────────────────────────

pub fn render_code(data: serde_json::Value) -> impl IntoView {
    let code = str_field_or(&data, "code", "content").unwrap_or_default();
    let language = str_field(&data, "language").unwrap_or_default();

    view! {
        <crate::components::code_block::CodeBlock language=language code=code />
    }
}

// ── Metric ───────────────────────────────────────────────────────

pub fn render_metric(data: serde_json::Value) -> impl IntoView {
    let label = str_field(&data, "label").unwrap_or_default();
    let value = str_field(&data, "value").unwrap_or_else(|| {
        data.get("value")
            .map(|v| cell_to_string(v))
            .unwrap_or_default()
    });
    let trend = str_field(&data, "trend");
    let description = str_field(&data, "description");

    let trend_cls = trend.as_deref().map(|t| match t {
        "up" => "a2ui-metric-trend-up",
        "down" => "a2ui-metric-trend-down",
        _ => "a2ui-metric-trend-flat",
    });

    view! {
        <div class="a2ui-metric">
            <span class="a2ui-metric-label">{label}</span>
            <span class="a2ui-metric-value">{value}</span>
            {trend_cls.map(|cls| view! {
                <span class=format!("a2ui-metric-trend {}", cls)>
                    {match trend.as_deref() {
                        Some("up") => "↑",
                        Some("down") => "↓",
                        _ => "→",
                    }}
                </span>
            })}
            {description.map(|d| view! { <span class="a2ui-metric-desc">{d}</span> })}
        </div>
    }
}
