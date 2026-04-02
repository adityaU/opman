//! Static A2UI block renderers — card, table, kv, status, progress, alert, markdown.

use crate::components::icons::*;
use crate::components::message_turn::{parse_markdown_segments, ContentSegment};
use leptos::prelude::*;

// ── Helpers ─────────────────────────────────────────────────────────

pub fn str_field(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Like str_field but tries a fallback key too.
fn str_field_or(data: &serde_json::Value, primary: &str, fallback: &str) -> Option<String> {
    str_field(data, primary).or_else(|| str_field(data, fallback))
}

fn f64_field_or(data: &serde_json::Value, primary: &str, fallback: &str) -> Option<f64> {
    data.get(primary)
        .and_then(|v| v.as_f64())
        .or_else(|| data.get(fallback).and_then(|v| v.as_f64()))
}

pub fn cell_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ── Block renderers ─────────────────────────────────────────────────

pub fn render_card(data: serde_json::Value) -> impl IntoView {
    let title = str_field(&data, "title").unwrap_or_default();
    let body = str_field_or(&data, "body", "content").unwrap_or_default();
    let icon = str_field(&data, "icon");

    view! {
        <div class="a2ui-card">
            <div class="a2ui-card-header">
                {icon.map(|i| view! { <span class="a2ui-card-icon">{i}</span> })}
                <span class="a2ui-card-title">{title}</span>
            </div>
            {(!body.is_empty()).then(|| view! {
                <div class="a2ui-card-body">{body}</div>
            })}
        </div>
    }
}

pub fn render_table(data: serde_json::Value) -> impl IntoView {
    let headers: Vec<String> = data
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let rows: Vec<Vec<String>> = data
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|row| {
                    row.as_array()
                        .map(|cells| cells.iter().map(|c| cell_to_string(c)).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();

    view! {
        <div class="a2ui-table-wrap">
            <table class="a2ui-table">
                {(!headers.is_empty()).then(|| view! {
                    <thead>
                        <tr>
                            {headers.iter().map(|h| view! {
                                <th>{h.clone()}</th>
                            }).collect_view()}
                        </tr>
                    </thead>
                })}
                <tbody>
                    {rows.iter().map(|row| view! {
                        <tr>
                            {row.iter().map(|cell| view! {
                                <td>{cell.clone()}</td>
                            }).collect_view()}
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

pub fn render_kv(data: serde_json::Value) -> impl IntoView {
    let pairs: Vec<(String, String)> = data
        .get("pairs")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let k = item.get("key")?.as_str()?.to_string();
                    let v = cell_to_string(item.get("value").unwrap_or(&serde_json::Value::Null));
                    Some((k, v))
                })
                .collect()
        })
        .unwrap_or_default();

    view! {
        <div class="a2ui-kv">
            {pairs.iter().map(|(k, v)| view! {
                <div class="a2ui-kv-row">
                    <span class="a2ui-kv-key">{k.clone()}</span>
                    <span class="a2ui-kv-val">{v.clone()}</span>
                </div>
            }).collect_view()}
        </div>
    }
}

pub fn render_status(data: serde_json::Value) -> impl IntoView {
    let label = str_field(&data, "label").unwrap_or("Status".into());
    let level = str_field(&data, "level").unwrap_or("info".into());
    let detail = str_field_or(&data, "detail", "message");
    let cls = format!("a2ui-status a2ui-status-{}", level);

    view! {
        <div class=cls>
            {level_icon(&level, 14)}
            <span class="a2ui-status-label">{label}</span>
            {detail.map(|d| view! { <span class="a2ui-status-detail">{d}</span> })}
        </div>
    }
}

pub fn render_progress(data: serde_json::Value) -> impl IntoView {
    let label = str_field(&data, "label").unwrap_or_default();
    let pct = f64_field_or(&data, "percent", "percentage")
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);
    let width_style = format!("width: {}%", pct);

    view! {
        <div class="a2ui-progress">
            <div class="a2ui-progress-header">
                <span class="a2ui-progress-label">{label}</span>
                <span class="a2ui-progress-pct">{format!("{:.0}%", pct)}</span>
            </div>
            <div class="a2ui-progress-track">
                <div class="a2ui-progress-fill" style=width_style></div>
            </div>
        </div>
    }
}

pub fn render_alert(data: serde_json::Value) -> impl IntoView {
    let message = str_field(&data, "message").unwrap_or_default();
    let level = str_field(&data, "level").unwrap_or("info".into());
    let cls = format!("a2ui-alert a2ui-alert-{}", level);

    view! {
        <div class=cls>
            {level_icon(&level, 14)}
            <span>{message}</span>
        </div>
    }
}

pub fn render_markdown(data: serde_json::Value) -> impl IntoView {
    let content = str_field(&data, "content").unwrap_or_default();
    let segments = parse_markdown_segments(&content);

    view! {
        <div class="a2ui-markdown">
            {segments.into_iter().map(|seg| match seg {
                ContentSegment::Html(html) => view! {
                    <div inner_html=html></div>
                }.into_any(),
                ContentSegment::FencedCode { language, code } => view! {
                    <crate::components::code_block::CodeBlock language=language code=code />
                }.into_any(),
            }).collect_view()}
        </div>
    }
}

// ── Shared icon helper ──────────────────────────────────────────────

fn level_icon(level: &str, size: u32) -> impl IntoView {
    match level {
        "success" => view! { <IconCheckCircle2 size=size /> }.into_any(),
        "warning" => view! { <IconAlertTriangle size=size /> }.into_any(),
        "error" => view! { <IconXCircle size=size /> }.into_any(),
        _ => view! { <IconInfo size=size /> }.into_any(),
    }
}
