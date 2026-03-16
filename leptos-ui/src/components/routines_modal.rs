//! RoutinesModal — CRUD for scheduled/manual routines.
//! Matches React `RoutinesModal.tsx`.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_delete, api_fetch, api_patch, api_post};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::hooks::use_keyboard::use_escape;
use crate::hooks::use_providers::ProviderCache;
use crate::types::api::{RoutineDefinition, RoutineRunRecord, RoutinesListResponse};

// ── Cron Presets ────────────────────────────────────────────────────

struct CronPreset {
    label: &'static str,
    cron: &'static str,
}

const CRON_PRESETS: &[CronPreset] = &[
    CronPreset { label: "Every 5 minutes", cron: "0 */5 * * * *" },
    CronPreset { label: "Every 15 minutes", cron: "0 */15 * * * *" },
    CronPreset { label: "Every 30 minutes", cron: "0 */30 * * * *" },
    CronPreset { label: "Every hour", cron: "0 0 * * * *" },
    CronPreset { label: "Every 2 hours", cron: "0 0 */2 * * *" },
    CronPreset { label: "Every 6 hours", cron: "0 0 */6 * * *" },
    CronPreset { label: "Daily at 9 AM", cron: "0 0 9 * * *" },
    CronPreset { label: "Daily at midnight", cron: "0 0 0 * * *" },
    CronPreset { label: "Weekdays at 9 AM", cron: "0 0 9 * * 1-5" },
    CronPreset { label: "Monday at 9 AM", cron: "0 0 9 * * 1" },
];

const TIMEZONE_OPTIONS: &[&str] = &[
    "UTC",
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "Europe/London",
    "Europe/Berlin",
    "Europe/Paris",
    "Asia/Tokyo",
    "Asia/Shanghai",
    "Asia/Kolkata",
    "Australia/Sydney",
    "Pacific/Auckland",
];

// ── Helpers ─────────────────────────────────────────────────────────

fn trigger_label(trigger: &str) -> &'static str {
    match trigger {
        "scheduled" | "cron" => "scheduled",
        "manual" => "manual",
        "on_session_idle" => "on session idle",
        _ => "unknown",
    }
}

fn trigger_badge_class(trigger: &str) -> String {
    let base = "routines-trigger-badge";
    match trigger {
        "scheduled" | "cron" => format!("{} routines-trigger-badge--scheduled", base),
        "manual" => format!("{} routines-trigger-badge--manual", base),
        "on_session_idle" => format!("{} routines-trigger-badge--on_session_idle", base),
        _ => base.to_string(),
    }
}

fn describe_cron(expr: &str) -> String {
    if expr.is_empty() {
        return String::new();
    }
    for p in CRON_PRESETS {
        if p.cron == expr {
            return p.label.to_string();
        }
    }
    expr.to_string()
}

fn get_schedule_summary(routine: &RoutineDefinition) -> String {
    match routine.trigger.as_str() {
        "manual" => "Manual only".to_string(),
        "on_session_idle" => "On idle".to_string(),
        "scheduled" => {
            if let Some(ref cron) = routine.cron_expr {
                describe_cron(cron)
            } else {
                "Scheduled".to_string()
            }
        }
        other => other.to_string(),
    }
}

fn get_target_summary(routine: &RoutineDefinition) -> String {
    if routine.action != "send_message" {
        return String::new();
    }
    if routine.target_mode.as_deref() == Some("new_session") {
        return "new session".to_string();
    }
    if let Some(ref sid) = routine.session_id {
        let short: String = sid.chars().take(8).collect();
        return format!("session {}", short);
    }
    "current session".to_string()
}

fn format_relative_time(iso: Option<&str>) -> String {
    let iso = match iso {
        Some(s) if !s.is_empty() => s,
        _ => return "\u{2014}".to_string(), // em dash
    };

    // Try to parse ISO date using js_sys
    let js_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    let ts = js_date.get_time();
    if ts.is_nan() {
        return iso.chars().take(16).collect::<String>().replace('T', " ");
    }

    let now = js_sys::Date::now();
    let diff_ms = ts - now;
    let abs = diff_ms.abs() as u64;
    let future = diff_ms > 0.0;

    if abs < 60_000 {
        if future { "in <1m".to_string() } else { "<1m ago".to_string() }
    } else if abs < 3_600_000 {
        let m = (abs as f64 / 60_000.0).round() as u64;
        if future { format!("in {}m", m) } else { format!("{}m ago", m) }
    } else if abs < 86_400_000 {
        let h = (abs as f64 / 3_600_000.0).round() as u64;
        if future { format!("in {}h", h) } else { format!("{}h ago", h) }
    } else {
        iso.chars().take(16).collect::<String>().replace('T', " ")
    }
}

fn format_date(iso: &str) -> String {
    iso.chars().take(16).collect::<String>().replace('T', " ")
}

// ── Cron field matching (matches React computeNextRuns) ─────────

/// Parse a single cron field against a value. Supports *, */n, a-b, a,b,c, exact values.
fn match_cron_field(field: &str, value: u32) -> bool {
    if field == "*" {
        return true;
    }
    // Step: */n
    if let Some(step_str) = field.strip_prefix("*/") {
        if let Ok(step) = step_str.parse::<u32>() {
            return step > 0 && value % step == 0;
        }
    }
    // List: a,b,c
    for part in field.split(',') {
        let part = part.trim();
        // Range: a-b
        if let Some((lo_str, hi_str)) = part.split_once('-') {
            if let (Ok(lo), Ok(hi)) = (lo_str.trim().parse::<u32>(), hi_str.trim().parse::<u32>()) {
                if value >= lo && value <= hi {
                    return true;
                }
                continue;
            }
        }
        // Exact
        if let Ok(v) = part.parse::<u32>() {
            if v == value {
                return true;
            }
        }
    }
    false
}

/// Compute next `count` run times for a 6-field cron expression.
/// Client-side approximation (matches React `computeNextRuns`).
fn compute_next_runs(cron_expr: &str, _timezone: &str, count: usize) -> Vec<js_sys::Date> {
    let fields: Vec<&str> = cron_expr.split_whitespace().collect();
    if fields.len() < 5 {
        return Vec::new();
    }
    // Support both 5-field and 6-field cron
    let (sec_field, min_field, hour_field, dom_field, mon_field, dow_field) = if fields.len() >= 6 {
        (fields[0], fields[1], fields[2], fields[3], fields[4], fields[5])
    } else {
        ("0", fields[0], fields[1], fields[2], fields[3], fields[4])
    };

    let now = js_sys::Date::new_0();
    // Start from next minute
    let mut candidate = js_sys::Date::new_0();
    candidate.set_full_year(now.get_full_year());
    candidate.set_month(now.get_month());
    candidate.set_date(now.get_date());
    candidate.set_hours(now.get_hours());
    candidate.set_minutes(now.get_minutes() + 1);
    candidate.set_seconds(0);
    candidate.set_milliseconds(0);

    let mut results = Vec::new();
    let max_iterations = 525_960u32; // ~1 year of minutes

    for _ in 0..max_iterations {
        if results.len() >= count {
            break;
        }

        let min = candidate.get_minutes();
        let hour = candidate.get_hours();
        let dom = candidate.get_date();
        let mon = candidate.get_month() + 1; // JS months are 0-based
        let dow = candidate.get_day(); // 0=Sunday

        if match_cron_field(sec_field, 0)
            && match_cron_field(min_field, min)
            && match_cron_field(hour_field, hour)
            && match_cron_field(dom_field, dom)
            && match_cron_field(mon_field, mon)
            && match_cron_field(dow_field, dow)
        {
            let d = js_sys::Date::new_0();
            d.set_time(candidate.get_time());
            results.push(d);
        }

        // Advance by 1 minute
        candidate.set_minutes(candidate.get_minutes() + 1);
    }

    results
}

/// Format a js_sys::Date for next-run display (matches React toLocaleString format).
fn format_next_run_date(d: &js_sys::Date) -> String {
    // Use toLocaleString via js_sys for locale-aware formatting
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"weekday".into(), &"short".into());
    let _ = js_sys::Reflect::set(&options, &"month".into(), &"short".into());
    let _ = js_sys::Reflect::set(&options, &"day".into(), &"numeric".into());
    let _ = js_sys::Reflect::set(&options, &"hour".into(), &"2-digit".into());
    let _ = js_sys::Reflect::set(&options, &"minute".into(), &"2-digit".into());
    d.to_locale_string(
        "",
        &options.into(),
    )
    .as_string()
    .unwrap_or_else(|| format!("{}", d.to_string()))
}

fn event_target_checked(e: &leptos::ev::Event) -> bool {
    use wasm_bindgen::JsCast;
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}

#[allow(dead_code)]
fn status_color(status: &str) -> &'static str {
    match status {
        "success" | "completed" => "var(--color-success, #4caf50)",
        "error" | "failed" => "var(--color-error, #e05252)",
        "running" => "var(--color-info, #5c8fff)",
        _ => "var(--color-text-muted, #999)",
    }
}

// ── API bodies ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateRoutineBody {
    name: String,
    trigger: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cron_expr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_id: Option<String>,
    enabled: bool,
}

#[derive(Serialize)]
struct UpdateRoutineBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cron_expr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    /// Explicit Option<Option<String>> to send null when clearing override.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_id: Option<Option<String>>,
}

#[derive(Serialize)]
struct RunRoutineBody {}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn RoutinesModal(
    on_close: Callback<()>,
    #[prop(optional)]
    active_session_id: Option<String>,
) -> impl IntoView {
    // ── Escape key ──
    use_escape(on_close);

    // ── Core state ──
    let (routines, set_routines) = signal(Vec::<RoutineDefinition>::new());
    let (runs, set_runs) = signal(Vec::<RoutineRunRecord>::new());
    let (loading, set_loading) = signal(true);
    let (expanded_id, set_expanded_id) = signal(Option::<String>::None);

    // ── Create/edit form ──
    let (creating, set_creating) = signal(false);
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (form_name, set_form_name) = signal(String::new());
    let (form_trigger, set_form_trigger) = signal("scheduled".to_string());
    let (form_cron, set_form_cron) = signal("0 0 */6 * * *".to_string());
    let (form_timezone, set_form_timezone) = signal("UTC".to_string());
    let (form_prompt, set_form_prompt) = signal(String::new());
    let (form_enabled, set_form_enabled) = signal(true);
    let (saving, set_saving) = signal(false);

    // ── Provider/model override ──
    let (form_provider_id, set_form_provider_id) = signal(String::new());
    let (form_model_id, set_form_model_id) = signal(String::new());
    let (show_model_override, set_show_model_override) = signal(false);

    // ── Cron mode ──
    let (cron_mode, set_cron_mode) = signal("preset".to_string());

    // ── Error state ──
    let (error_msg, set_error_msg) = signal(Option::<String>::None);

    // ── Running state ──
    let (running_id, set_running_id) = signal(Option::<String>::None);

    // ── Confirm delete ──
    let (confirm_delete_id, set_confirm_delete_id) = signal(Option::<String>::None);

    // ── Name input ref for autofocus ──
    let name_input_ref = NodeRef::<leptos::html::Input>::new();

    // ── Reset form helper ──
    let reset_form = move || {
        set_form_name.set(String::new());
        set_form_trigger.set("scheduled".to_string());
        set_form_cron.set("0 0 */6 * * *".to_string());
        set_form_timezone.set("UTC".to_string());
        set_form_prompt.set(String::new());
        set_form_enabled.set(true);
        set_editing_id.set(None);
        set_cron_mode.set("preset".to_string());
        set_form_provider_id.set(String::new());
        set_form_model_id.set(String::new());
        set_show_model_override.set(false);
    };

    // ── Autofocus effect ──
    Effect::new(move |_| {
        let show = creating.get() || editing_id.get().is_some();
        if show {
            if let Some(el) = name_input_ref.get() {
                let _ = el.focus();
            }
        }
    });

    // ── Load on mount ──
    {
        leptos::task::spawn_local(async move {
            match api_fetch::<RoutinesListResponse>("/routines").await {
                Ok(resp) => {
                    set_routines.set(resp.routines);
                    set_runs.set(resp.runs);
                }
                Err(e) => leptos::logging::warn!("Failed to load routines: {}", e),
            }
            set_loading.set(false);
        });
    }

    // ── SSE refresh: re-fetch routines on "opman:routine-updated" DOM event ──
    // (Matches React RoutinesModal.tsx useEffect on "opman:routine-updated")
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        let handler = Closure::<dyn Fn()>::new(move || {
            leptos::task::spawn_local(async move {
                match api_fetch::<RoutinesListResponse>("/routines").await {
                    Ok(resp) => {
                        set_routines.set(resp.routines);
                        set_runs.set(resp.runs);
                    }
                    Err(e) => leptos::logging::warn!("Failed to refresh routines on SSE: {}", e),
                }
            });
        });
        let window = web_sys::window().unwrap();
        let _ = window.add_event_listener_with_callback(
            "opman:routine-updated",
            handler.as_ref().unchecked_ref(),
        );
        let handler_fn = handler.as_ref().unchecked_ref::<js_sys::Function>().clone();
        on_cleanup(move || {
            if let Some(w) = web_sys::window() {
                let _ = w.remove_event_listener_with_callback(
                    "opman:routine-updated",
                    &handler_fn,
                );
            }
        });
        handler.forget();
    }

    // ── Derive: is the form showing? ──
    let show_form = Memo::new(move |_| creating.get() || editing_id.get().is_some());

    // ── Derive: form title ──
    let form_title = Memo::new(move |_| {
        if editing_id.get().is_some() {
            "Edit Routine"
        } else {
            "New Routine"
        }
    });

    // ── Derive: submit disabled ──
    let submit_disabled = Memo::new(move |_| {
        saving.get()
            || form_name.get().trim().is_empty()
            || form_prompt.get().trim().is_empty()
    });

    // ── Derive: submit button text ──
    let submit_text = Memo::new(move |_| {
        if editing_id.get().is_some() {
            "Save Changes"
        } else {
            "Create Routine"
        }
    });

    // ── Handle create ──
    let handle_create = Callback::new(move |_: ()| {
        let n = form_name.get_untracked();
        if n.trim().is_empty() {
            return;
        }
        let p = form_prompt.get_untracked();
        if p.trim().is_empty() {
            return;
        }
        let t = form_trigger.get_untracked();
        let cron = form_cron.get_untracked();
        let tz = form_timezone.get_untracked();
        let enabled = form_enabled.get_untracked();
        let active_sid = active_session_id.clone();
        let pid = form_provider_id.get_untracked();
        let mid = form_model_id.get_untracked();

        set_saving.set(true);
        set_error_msg.set(None);
        leptos::task::spawn_local(async move {
            let body = CreateRoutineBody {
                name: n.trim().to_string(),
                trigger: t.clone(),
                action: "send_message".to_string(),
                cron_expr: if t == "scheduled" && !cron.is_empty() { Some(cron) } else { None },
                timezone: if t == "scheduled" { Some(tz) } else { None },
                prompt: if !p.is_empty() { Some(p) } else { None },
                target_mode: Some("existing_session".to_string()),
                session_id: active_sid,
                project_index: None,
                provider_id: if !pid.is_empty() { Some(pid) } else { None },
                model_id: if !mid.is_empty() { Some(mid) } else { None },
                enabled,
            };
            match api_post::<RoutineDefinition>("/routines", &body).await {
                Ok(created) => {
                    set_routines.update(|list| list.insert(0, created));
                    reset_form();
                    set_creating.set(false);
                }
                Err(e) => {
                    set_error_msg.set(Some(format!("Failed to create routine: {}", e)));
                }
            }
            set_saving.set(false);
        });
    });

    // ── Handle save edit ──
    let handle_save_edit = Callback::new(move |_: ()| {
        let eid = match editing_id.get_untracked() {
            Some(id) => id,
            None => return,
        };
        let n = form_name.get_untracked();
        if n.trim().is_empty() {
            return;
        }
        let p = form_prompt.get_untracked();
        let t = form_trigger.get_untracked();
        let cron = form_cron.get_untracked();
        let tz = form_timezone.get_untracked();
        let enabled = form_enabled.get_untracked();
        let pid = form_provider_id.get_untracked();
        let mid = form_model_id.get_untracked();

        set_saving.set(true);
        set_error_msg.set(None);
        leptos::task::spawn_local(async move {
            let path = format!("/routines/{}", eid);
            let body = UpdateRoutineBody {
                name: Some(n.trim().to_string()),
                trigger: Some(t.clone()),
                action: Some("send_message".to_string()),
                cron_expr: if t == "scheduled" { Some(cron) } else { None },
                timezone: if t == "scheduled" { Some(tz) } else { None },
                prompt: if !p.is_empty() { Some(p) } else { None },
                target_mode: None,
                session_id: None,
                project_index: None,
                enabled: Some(enabled),
                provider_id: Some(if !pid.is_empty() { Some(pid) } else { None }),
                model_id: Some(if !mid.is_empty() { Some(mid) } else { None }),
            };
            match api_patch::<RoutineDefinition>(&path, &body).await {
                Ok(updated) => {
                    set_routines.update(|list| {
                        if let Some(r) = list.iter_mut().find(|r| r.id == updated.id) {
                            *r = updated;
                        }
                    });
                    reset_form();
                }
                Err(e) => {
                    set_error_msg.set(Some(format!("Failed to save routine: {}", e)));
                }
            }
            set_saving.set(false);
        });
    });

    // ── Handle run ──
    let handle_run = Callback::new(move |rid: String| {
        set_running_id.set(Some(rid.clone()));
        set_error_msg.set(None);
        leptos::task::spawn_local(async move {
            let path = format!("/routines/{}/run", rid);
            let body = RunRoutineBody {};
            match api_post::<RoutineRunRecord>(&path, &body).await {
                Ok(record) => {
                    set_runs.update(|list| list.insert(0, record));
                }
                Err(e) => {
                    set_error_msg.set(Some(format!("Failed to run routine: {}", e)));
                }
            }
            set_running_id.set(None);
        });
    });

    // ── Handle toggle enabled ──
    let handle_toggle_enabled = Callback::new(move |(rid, enabled): (String, bool)| {
        set_error_msg.set(None);
        leptos::task::spawn_local(async move {
            let path = format!("/routines/{}", rid);
            let body = UpdateRoutineBody {
                name: None,
                trigger: None,
                action: None,
                cron_expr: None,
                timezone: None,
                prompt: None,
                target_mode: None,
                session_id: None,
                project_index: None,
                enabled: Some(enabled),
                provider_id: None,
                model_id: None,
            };
            match api_patch::<RoutineDefinition>(&path, &body).await {
                Ok(updated) => {
                    set_routines.update(|list| {
                        if let Some(r) = list.iter_mut().find(|r| r.id == updated.id) {
                            *r = updated;
                        }
                    });
                }
                Err(e) => {
                    set_error_msg.set(Some(format!("Failed to toggle routine: {}", e)));
                }
            }
        });
    });

    // ── Handle delete ──
    let handle_delete = Callback::new(move |rid: String| {
        set_error_msg.set(None);
        set_confirm_delete_id.set(None);
        let rid2 = rid.clone();
        leptos::task::spawn_local(async move {
            let path = format!("/routines/{}", rid2);
            match api_delete(&path).await {
                Ok(_) => {
                    set_routines.update(|list| list.retain(|r| r.id != rid2));
                    // If we were editing this routine, cancel the edit
                    if editing_id.get_untracked().as_ref() == Some(&rid2) {
                        reset_form();
                    }
                }
                Err(e) => {
                    set_error_msg.set(Some(format!("Failed to delete routine: {}", e)));
                }
            }
        });
    });

    // ── Start edit ──
    let start_edit = Callback::new(move |routine: RoutineDefinition| {
        set_form_name.set(routine.name.clone());
        set_form_trigger.set(routine.trigger.clone());
        set_form_cron.set(routine.cron_expr.clone().unwrap_or_default());
        set_form_timezone.set(routine.timezone.clone().unwrap_or_else(|| "UTC".to_string()));
        set_form_prompt.set(routine.prompt.clone().unwrap_or_default());
        set_form_enabled.set(routine.enabled);
        set_form_provider_id.set(routine.provider_id.clone().unwrap_or_default());
        set_form_model_id.set(routine.model_id.clone().unwrap_or_default());
        set_show_model_override.set(routine.provider_id.is_some() || routine.model_id.is_some());
        set_editing_id.set(Some(routine.id.clone()));
        set_creating.set(false);
        set_error_msg.set(None);
        // Determine cron mode
        let cron_val = routine.cron_expr.as_deref().unwrap_or("");
        let is_preset = CRON_PRESETS.iter().any(|p| p.cron == cron_val);
        set_cron_mode.set(if is_preset { "preset" } else { "custom" }.to_string());
    });

    // ── Cancel edit / create ──
    let cancel_form = move || {
        reset_form();
        set_creating.set(false);
        set_error_msg.set(None);
    };

    view! {
        <ModalOverlay on_close=on_close class="routines-modal">
            // Header
            <div class="routines-header">
                <div class="routines-header-left">
                    // Clock3 icon (inline SVG since no IconClock3 exists)
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="10" />
                        <polyline points="12 6 12 12 16.5 12" />
                    </svg>
                    <h3>"Routines"</h3>
                    <span class="routines-count">{move || routines.get().len()}</span>
                </div>
                <div class="routines-header-actions">
                    <button
                        class="routines-add-btn"
                        title="Create routine"
                        on:click=move |_| {
                            reset_form();
                            set_creating.update(|v| *v = !*v);
                        }
                    >
                        <IconPlus size=14 />
                        " New"
                    </button>
                    <button on:click=move |_| on_close.run(()) aria-label="Close routines">
                        <IconX size=16 />
                    </button>
                </div>
            </div>

            // Error banner (global, outside scroll)
            {move || error_msg.get().map(|msg| {
                view! {
                    <div class="routines-error-banner">
                        <IconAlertTriangle size=13 />
                        <span>{msg}</span>
                        <button class="routines-error-dismiss" on:click=move |_| set_error_msg.set(None)>
                            "\u{00D7}"
                        </button>
                    </div>
                }
            })}

            // Scrollable content area
            <div class="routines-scrollable">

                // ── Create / Edit Form ──
                {move || show_form.get().then(|| {
                    let _is_scheduled = form_trigger.get() == "scheduled";
                    let trigger_val = form_trigger.get();
                    let is_idle = trigger_val == "on_session_idle";

                    view! {
                        <div class="routines-form">
                            <div class="routines-form-title">{move || form_title.get()}</div>

                            // Section: Basics
                            <div class="routines-form-section">
                                <div class="routines-form-section-header">"Basics"</div>
                                <input
                                    class="routines-input"
                                    placeholder="Routine name"
                                    node_ref=name_input_ref
                                    prop:value=move || form_name.get()
                                    on:input=move |ev| set_form_name.set(event_target_value(&ev))
                                />
                                <div class="routines-form-group">
                                    <label class="routines-label">"Trigger"</label>
                                    <select
                                        class="routines-select"
                                        prop:value=move || form_trigger.get()
                                        on:change=move |ev| set_form_trigger.set(event_target_value(&ev))
                                    >
                                        <option value="scheduled">"Scheduled (Cron)"</option>
                                        <option value="manual">"Manual"</option>
                                        {is_idle.then(|| view! {
                                            <option value="on_session_idle">"On Session Idle (legacy)"</option>
                                        })}
                                    </select>
                                </div>
                            </div>

                            // Section: Schedule (only if trigger=scheduled)
                            {move || (form_trigger.get() == "scheduled").then(|| view! {
                                <div class="routines-form-section">
                                    <div class="routines-form-section-header">"Schedule"</div>
                                    <div class="routines-cron-section">
                                        <div class="routines-form-row">
                                            <div class="routines-form-group routines-form-group--wide">
                                                <div class="routines-cron-tabs">
                                                    <button
                                                        class=move || if cron_mode.get() == "preset" { "routines-cron-tab active" } else { "routines-cron-tab" }
                                                        on:click=move |_| set_cron_mode.set("preset".to_string())
                                                    >
                                                        "Presets"
                                                    </button>
                                                    <button
                                                        class=move || if cron_mode.get() == "custom" { "routines-cron-tab active" } else { "routines-cron-tab" }
                                                        on:click=move |_| set_cron_mode.set("custom".to_string())
                                                    >
                                                        "Raw"
                                                    </button>
                                                </div>
                                            </div>
                                            <div class="routines-form-group">
                                                <label class="routines-label">"Timezone"</label>
                                                <select
                                                    class="routines-select"
                                                    prop:value=move || form_timezone.get()
                                                    on:change=move |ev| set_form_timezone.set(event_target_value(&ev))
                                                >
                                                    {TIMEZONE_OPTIONS.iter().map(|tz| {
                                                        let tz_val = tz.to_string();
                                                        let tz_display = tz.to_string();
                                                        view! { <option value=tz_val>{tz_display}</option> }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                            </div>
                                        </div>

                                        // Preset mode
                                        {move || (cron_mode.get() == "preset").then(|| {
                                            let current_cron = form_cron.get();
                                            view! {
                                                <div class="routines-cron-presets">
                                                    {CRON_PRESETS.iter().map(|preset| {
                                                        let _cron_val = preset.cron.to_string();
                                                        let cron_val2 = preset.cron.to_string();
                                                        let label = preset.label;
                                                        let is_active = current_cron == preset.cron;
                                                        let cls = if is_active { "routines-cron-preset active" } else { "routines-cron-preset" };
                                                        view! {
                                                            <button
                                                                class=cls
                                                                on:click=move |_| set_form_cron.set(cron_val2.clone())
                                                            >
                                                                {label}
                                                            </button>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </div>
                                            }
                                        })}

                                        // Raw / custom mode
                                        {move || (cron_mode.get() == "custom").then(|| view! {
                                            <div class="routines-cron-custom">
                                                <input
                                                    class="routines-input routines-cron-input"
                                                    placeholder="sec min hour dom month dow (e.g. 0 0 9 * * 1-5)"
                                                    prop:value=move || form_cron.get()
                                                    on:input=move |ev| set_form_cron.set(event_target_value(&ev))
                                                />
                                                <span class="routines-cron-hint">
                                                    "6-field cron: sec min hour day-of-month month day-of-week"
                                                </span>
                                                {move || {
                                                    let c = form_cron.get();
                                                    (!c.is_empty()).then(|| {
                                                        let desc = describe_cron(&c);
                                                        view! {
                                                            <span class="routines-cron-hint">
                                                                "Reads as: " {desc}
                                                            </span>
                                                        }
                                                    })
                                                }}
                                            </div>
                                        })}

                                        // Next 5 runs preview
                                        {move || {
                                            let cron = form_cron.get();
                                            let tz = form_timezone.get();
                                            if cron.is_empty() {
                                                return None;
                                            }
                                            let next_runs = compute_next_runs(&cron, &tz, 5);
                                            if next_runs.is_empty() {
                                                return None;
                                            }
                                            let count = next_runs.len();
                                            let items = next_runs.iter().map(|d| {
                                                let formatted = format_next_run_date(d);
                                                view! {
                                                    <li class="routines-next-runs-item">{formatted}</li>
                                                }
                                            }).collect::<Vec<_>>();
                                            Some(view! {
                                                <div class="routines-next-runs">
                                                    <div class="routines-next-runs-title">
                                                        "Next " {count} " runs"
                                                    </div>
                                                    <ul class="routines-next-runs-list">
                                                        {items}
                                                    </ul>
                                                </div>
                                            })
                                        }}
                                    </div>
                                </div>
                            })}

                            // Section: Message
                            <div class="routines-form-section">
                                <div class="routines-form-section-header">"Message"</div>
                                <textarea
                                    class="routines-textarea"
                                    rows="3"
                                    placeholder="Enter the message to send to the session..."
                                    prop:value=move || form_prompt.get()
                                    on:input=move |ev| set_form_prompt.set(event_target_value(&ev))
                                />
                            </div>

                            // Section: Model Override (collapsed by default)
                            <div class="routines-form-section">
                                <label class="routines-form-section-toggle">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || show_model_override.get()
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_show_model_override.set(checked);
                                            if !checked {
                                                set_form_provider_id.set(String::new());
                                                set_form_model_id.set(String::new());
                                            }
                                        }
                                    />
                                    <span class="routines-form-section-header routines-form-section-header--toggle">
                                        "Customize model"
                                    </span>
                                </label>
                                {move || show_model_override.get().then(|| {
                                    let providers = expect_context::<ProviderCache>();
                                    let all_providers = providers.all.get();
                                    let connected = providers.connected.get();
                                    let provider_options: Vec<_> = all_providers.iter()
                                        .filter(|p| connected.contains(&p.id))
                                        .cloned()
                                        .collect();
                                    let current_pid = form_provider_id.get();
                                    let model_options: Vec<_> = if current_pid.is_empty() {
                                        Vec::new()
                                    } else {
                                        all_providers.iter()
                                            .find(|p| p.id == current_pid)
                                            .map(|p| p.models.values().cloned().collect::<Vec<_>>())
                                            .unwrap_or_default()
                                    };
                                    let has_provider = !current_pid.is_empty();

                                    view! {
                                        <div class="routines-form-row">
                                            <div class="routines-form-group">
                                                <label class="routines-label">"Provider"</label>
                                                <select
                                                    class="routines-select"
                                                    prop:value=move || form_provider_id.get()
                                                    on:change=move |ev| {
                                                        set_form_provider_id.set(event_target_value(&ev));
                                                        set_form_model_id.set(String::new());
                                                    }
                                                >
                                                    <option value="">"Default provider"</option>
                                                    {provider_options.iter().map(|p| {
                                                        let id = p.id.clone();
                                                        let display = if let Some(ref name) = Some(&p.name) {
                                                            if !name.is_empty() { name.to_string() } else { p.id.clone() }
                                                        } else {
                                                            p.id.clone()
                                                        };
                                                        view! { <option value=id>{display}</option> }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                            </div>
                                            <div class="routines-form-group">
                                                <label class="routines-label">"Model"</label>
                                                <select
                                                    class="routines-select"
                                                    prop:value=move || form_model_id.get()
                                                    on:change=move |ev| set_form_model_id.set(event_target_value(&ev))
                                                    disabled=move || form_provider_id.get().is_empty()
                                                >
                                                    <option value="">"Default model"</option>
                                                    {model_options.iter().map(|m| {
                                                        let id = m.id.clone();
                                                        let display = m.name.as_deref().unwrap_or(&m.id).to_string();
                                                        view! { <option value=id>{display}</option> }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                                {(!has_provider).then(|| view! {
                                                    <span class="routines-cron-hint">"Select a provider first"</span>
                                                })}
                                            </div>
                                        </div>
                                    }
                                })}
                            </div>

                            // Form footer: enabled toggle + actions
                            <div class="routines-form-footer">
                                <label class="routines-toggle">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || form_enabled.get()
                                        on:change=move |ev| set_form_enabled.set(event_target_checked(&ev))
                                    />
                                    <span class="routines-toggle-label">
                                        {move || if form_enabled.get() { "Enabled" } else { "Disabled" }}
                                    </span>
                                </label>
                                <div class="routines-form-actions">
                                    <button
                                        class="routines-btn routines-btn--muted"
                                        on:click=move |_| cancel_form()
                                    >
                                        "Cancel"
                                    </button>
                                    <button
                                        class="routines-btn routines-btn--primary"
                                        disabled=move || submit_disabled.get()
                                        on:click=move |_| {
                                            if editing_id.get_untracked().is_some() {
                                                handle_save_edit.run(());
                                            } else {
                                                handle_create.run(());
                                            }
                                        }
                                    >
                                        {move || saving.get().then(|| view! { <span class="routines-spinner"></span> })}
                                        <IconCheck size=14 />
                                        " "
                                        {move || submit_text.get()}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                })}

                // ── Routine list ──
                <div class="routines-body">
                    {move || {
                        if loading.get() {
                            return view! { <div class="routines-empty">"Loading routines..."</div> }.into_any();
                        }
                        let all = routines.get();
                        if all.is_empty() && !show_form.get() {
                            return view! {
                                <div class="routines-empty-state">
                                    // Calendar icon (inline SVG)
                                    <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                                        <rect width="18" height="18" x="3" y="4" rx="2" ry="2" />
                                        <line x1="16" x2="16" y1="2" y2="6" />
                                        <line x1="8" x2="8" y1="2" y2="6" />
                                        <line x1="3" x2="21" y1="10" y2="10" />
                                    </svg>
                                    <div class="routines-empty-title">"No routines yet"</div>
                                    <div class="routines-empty-desc">
                                        "Create a routine to automatically send messages to sessions on a schedule."
                                    </div>
                                    <button
                                        class="routines-btn routines-btn--primary"
                                        on:click=move |_| {
                                            reset_form();
                                            set_creating.set(true);
                                        }
                                    >
                                        <IconPlus size=14 />
                                        " Create your first routine"
                                    </button>
                                </div>
                            }.into_any();
                        }

                        let all_runs = runs.get();
                        let cards = all.into_iter().map(|routine| {
                            let rid = routine.id.clone();
                            let rid_expand = routine.id.clone();
                            let rid_expand2 = routine.id.clone();
                            let rid_toggle = routine.id.clone();
                            let rid_run = routine.id.clone();
                            let rid_delete = routine.id.clone();
                            let rid_detail = routine.id.clone();
                            let rid_editing_check = routine.id.clone();
                            let rid_running_check = routine.id.clone();
                            let name = routine.name.clone();
                            let _name_for_meta = routine.name.clone();
                            let trigger = routine.trigger.clone();
                            let action = routine.action.clone();
                            let is_enabled = routine.enabled;
                            let prompt_preview: String = routine.prompt.as_deref().unwrap_or("").chars().take(60).collect();
                            let has_more_prompt = routine.prompt.as_deref().map(|p| p.len() > 60).unwrap_or(false);
                            let prompt_full = routine.prompt.clone();
                            let cron_text = routine.cron_expr.clone().unwrap_or_default();
                            let tz_text = routine.timezone.clone().unwrap_or_default();
                            let last_error = routine.last_error.clone();
                            let last_error_detail = routine.last_error.clone();
                            let schedule_summary = get_schedule_summary(&routine);
                            let target_summary = get_target_summary(&routine);
                            let badge_class = trigger_badge_class(&trigger);
                            let trig_label = trigger_label(&trigger).to_string();

                            // Target detail string
                            let target_detail = if routine.action == "send_message" {
                                if routine.target_mode.as_deref() == Some("new_session") {
                                    format!("New session (project #{})", routine.project_index.unwrap_or(0))
                                } else if let Some(ref sid) = routine.session_id {
                                    let short: String = sid.chars().take(12).collect();
                                    format!("Session {}...", short)
                                } else {
                                    "Current session".to_string()
                                }
                            } else {
                                String::new()
                            };

                            // Provider/model detail
                            let provider_model = {
                                let mut s = String::new();
                                if let Some(ref pid) = routine.provider_id {
                                    s.push_str(pid);
                                    s.push('/');
                                }
                                if let Some(ref mid) = routine.model_id {
                                    s.push_str(mid);
                                } else if !s.is_empty() {
                                    s.push_str("default");
                                }
                                s
                            };
                            let has_provider_model = !provider_model.is_empty();

                            let last_run_at = routine.last_run_at.clone();
                            let next_run_at = routine.next_run_at.clone();
                            let routine_trigger = routine.trigger.clone();

                            let routine_runs: Vec<_> = all_runs.iter()
                                .filter(|r| r.routine_id == rid)
                                .take(5)
                                .cloned()
                                .collect();
                            let has_runs = !routine_runs.is_empty();

                            // Clone routine for edit
                            let routine_for_edit = routine.clone();

                            // Card classes
                            let card_class = move || {
                                let mut cls = "routines-card".to_string();
                                if !is_enabled {
                                    cls.push_str(" routines-card--disabled");
                                }
                                if editing_id.get().as_ref() == Some(&rid_editing_check) {
                                    cls.push_str(" routines-card--editing");
                                }
                                cls
                            };

                            view! {
                                <div class=card_class>
                                    <div class="routines-card-header">
                                        <button
                                            class="routines-card-expand"
                                            on:click=move |_| {
                                                set_expanded_id.update(|eid| {
                                                    if eid.as_ref() == Some(&rid_expand) {
                                                        *eid = None;
                                                    } else {
                                                        *eid = Some(rid_expand.clone());
                                                    }
                                                });
                                            }
                                        >
                                            {move || if expanded_id.get().as_ref() == Some(&rid_expand2) {
                                                view! { <IconChevronDown size=14 /> }.into_any()
                                            } else {
                                                view! { <IconChevronRight size=14 /> }.into_any()
                                            }}
                                        </button>

                                        <div class="routines-card-info"
                                            on:click={
                                                let rid_info = rid_detail.clone();
                                                move |_: web_sys::MouseEvent| {
                                                    set_expanded_id.update(|eid| {
                                                        if eid.as_ref() == Some(&rid_info) {
                                                            *eid = None;
                                                        } else {
                                                            *eid = Some(rid_info.clone());
                                                        }
                                                    });
                                                }
                                            }
                                        >
                                            <div class="routines-card-name">
                                                {if action == "send_message" {
                                                    view! { <IconSend size=13 /> }.into_any()
                                                } else {
                                                    // Clock icon for non-send_message
                                                    view! {
                                                        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <circle cx="12" cy="12" r="10" />
                                                            <polyline points="12 6 12 12 16.5 12" />
                                                        </svg>
                                                    }.into_any()
                                                }}
                                                " "
                                                {name.clone()}
                                            </div>
                                            {(!prompt_preview.is_empty()).then(|| {
                                                let pp = if has_more_prompt {
                                                    format!("{}…", prompt_preview)
                                                } else {
                                                    prompt_preview.clone()
                                                };
                                                view! { <div class="routines-card-prompt-preview">{pp}</div> }
                                            })}
                                            <div class="routines-card-meta">
                                                <span class=badge_class.clone()>{trig_label.clone()}</span>
                                                <span class="routines-schedule-summary">{schedule_summary.clone()}</span>
                                                {(!target_summary.is_empty()).then(|| {
                                                    let ts = target_summary.clone();
                                                    view! { <span class="routines-target-summary">{ts}</span> }
                                                })}
                                                {last_error.as_ref().map(|_| view! {
                                                    <span class="routines-error-badge">
                                                        <IconAlertTriangle size=11 />
                                                        " error"
                                                    </span>
                                                })}
                                            </div>
                                        </div>

                                        // Action buttons
                                        <div class="routines-card-actions">
                                            // Toggle enabled
                                            <button
                                                class=if is_enabled {
                                                    "routines-icon-btn routines-icon-btn--enabled"
                                                } else {
                                                    "routines-icon-btn routines-icon-btn--disabled-state"
                                                }
                                                title=if is_enabled { "Disable" } else { "Enable" }
                                                on:click={
                                                    let rid_t = rid_toggle.clone();
                                                    let new_state = !is_enabled;
                                                    move |_: web_sys::MouseEvent| {
                                                        handle_toggle_enabled.run((rid_t.clone(), new_state));
                                                    }
                                                }
                                            >
                                                {if is_enabled {
                                                    // Power icon (inline SVG)
                                                    view! {
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
                                                            <line x1="12" x2="12" y1="2" y2="12" />
                                                        </svg>
                                                    }.into_any()
                                                } else {
                                                    // PowerOff icon (inline SVG)
                                                    view! {
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <path d="M18.36 6.64A9 9 0 0 1 20.77 15" />
                                                            <path d="M6.16 6.16a9 9 0 1 0 12.68 12.68" />
                                                            <path d="M12 2v4" />
                                                            <path d="m2 2 20 20" />
                                                        </svg>
                                                    }.into_any()
                                                }}
                                            </button>

                                            // Edit button
                                            <button
                                                class="routines-icon-btn"
                                                title="Edit"
                                                on:click={
                                                    let r = routine_for_edit.clone();
                                                    move |_: web_sys::MouseEvent| {
                                                        start_edit.run(r.clone());
                                                    }
                                                }
                                            >
                                                <IconPencil size=14 />
                                            </button>

                                            // Run button
                                            <button
                                                class="routines-icon-btn routines-icon-btn--run"
                                                title="Run now"
                                                disabled=move || running_id.get().as_ref() == Some(&rid_running_check)
                                                on:click={
                                                    let rid_r = rid_run.clone();
                                                    move |_: web_sys::MouseEvent| handle_run.run(rid_r.clone())
                                                }
                                            >
                                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <polygon points="5 3 19 12 5 21 5 3" />
                                                </svg>
                                            </button>

                                            // Delete button
                                            <button
                                                class="routines-icon-btn routines-icon-btn--danger"
                                                title="Delete"
                                                on:click={
                                                    let rid_d = rid_delete.clone();
                                                    move |_: web_sys::MouseEvent| set_confirm_delete_id.set(Some(rid_d.clone()))
                                                }
                                            >
                                                <IconTrash2 size=14 />
                                            </button>
                                        </div>
                                    </div>

                                    // Expanded detail
                                    {move || {
                                        let is_expanded = expanded_id.get().as_ref() == Some(&rid);
                                        if !is_expanded {
                                            return None;
                                        }

                                        let schedule_row = if routine_trigger == "scheduled" {
                                            let cron_display = if !cron_text.is_empty() { cron_text.clone() } else { "\u{2014}".to_string() };
                                            let tz_display = if !tz_text.is_empty() { format!(" ({})", tz_text) } else { String::new() };
                                            Some(view! {
                                                <div class="routines-detail-row">
                                                    <span class="routines-detail-label">"Schedule"</span>
                                                    <span class="routines-detail-value">{cron_display}{tz_display}</span>
                                                </div>
                                            })
                                        } else { None };

                                        let target_row = if !target_detail.is_empty() {
                                            let td = target_detail.clone();
                                            Some(view! {
                                                <div class="routines-detail-row">
                                                    <span class="routines-detail-label">"Target"</span>
                                                    <span class="routines-detail-value">{td}</span>
                                                </div>
                                            })
                                        } else { None };

                                        let prompt_row = prompt_full.as_ref().map(|p| {
                                            let p = p.clone();
                                            view! {
                                                <div class="routines-detail-row routines-detail-row--block">
                                                    <span class="routines-detail-label">"Prompt"</span>
                                                    <div class="routines-detail-prompt">{p}</div>
                                                </div>
                                            }
                                        });

                                        let model_row = if has_provider_model {
                                            let pm = provider_model.clone();
                                            Some(view! {
                                                <div class="routines-detail-row">
                                                    <span class="routines-detail-label">"Model"</span>
                                                    <span class="routines-detail-value">{pm}</span>
                                                </div>
                                            })
                                        } else { None };

                                        let last_run_display = format_relative_time(last_run_at.as_deref());
                                        let next_run_display = format_relative_time(next_run_at.as_deref());

                                        let error_row = last_error_detail.as_ref().map(|err| {
                                            let e = err.clone();
                                            view! {
                                                <div class="routines-detail-row routines-detail-row--error">
                                                    <span class="routines-detail-label">"Last error"</span>
                                                    <span class="routines-detail-value routines-detail-value--error">{e}</span>
                                                </div>
                                            }
                                        });

                                        let runs_section = if has_runs {
                                            let rows = routine_runs.iter().map(|run| {
                                                let status = run.status.clone();
                                                let summary_text = if run.summary.len() > 60 {
                                                    let short: String = run.summary.chars().take(60).collect();
                                                    format!("{}...", short)
                                                } else {
                                                    run.summary.clone()
                                                };
                                                let summary_full = run.summary.clone();
                                                let dur = run.duration_ms.map(|ms| format!("{}ms", ms)).unwrap_or_default();
                                                let has_dur = run.duration_ms.is_some();
                                                let time_str = format_date(&run.created_at);
                                                let status_class = match status.as_str() {
                                                    "completed" | "success" => "routines-run-status routines-run-status--success",
                                                    "failed" | "error" => "routines-run-status routines-run-status--error",
                                                    _ => "routines-run-status",
                                                };
                                                let is_completed = status == "completed" || status == "success";
                                                let is_failed = status == "failed" || status == "error";

                                                view! {
                                                    <div class="routines-run-row">
                                                        <span class=status_class>
                                                            {is_completed.then(|| view! { <IconCheck size=11 /> })}
                                                            {is_failed.then(|| view! { <IconAlertTriangle size=11 /> })}
                                                            " "
                                                            {status}
                                                        </span>
                                                        <span class="routines-run-summary" title=summary_full>{summary_text}</span>
                                                        {has_dur.then(|| {
                                                            let d = dur.clone();
                                                            view! { <span class="routines-run-duration">{d}</span> }
                                                        })}
                                                        <span class="routines-run-time">{time_str}</span>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>();

                                            Some(view! {
                                                <div class="routines-runs-section">
                                                    <div class="routines-runs-title">"Recent Runs"</div>
                                                    {rows}
                                                </div>
                                            })
                                        } else { None };

                                        Some(view! {
                                            <div class="routines-card-detail">
                                                {schedule_row}
                                                {target_row}
                                                {prompt_row}
                                                {model_row}
                                                <div class="routines-detail-row">
                                                    <span class="routines-detail-label">"Last run"</span>
                                                    <span class="routines-detail-value">{last_run_display}</span>
                                                </div>
                                                <div class="routines-detail-row">
                                                    <span class="routines-detail-label">"Next run"</span>
                                                    <span class="routines-detail-value">{next_run_display}</span>
                                                </div>
                                                {error_row}
                                                {runs_section}
                                            </div>
                                        })
                                    }}
                                </div>
                            }
                        }).collect::<Vec<_>>();

                        view! { <div>{cards}</div> }.into_any()
                    }}
                </div>
            </div>

            // ── Delete confirmation dialog (outside scrollable) ──
            {move || confirm_delete_id.get().map(|del_id| {
                let del_id2 = del_id.clone();
                let del_name = routines.get().iter()
                    .find(|r| r.id == del_id)
                    .map(|r| r.name.clone())
                    .unwrap_or_default();
                view! {
                    <div class="routines-confirm-overlay" on:click=move |_| set_confirm_delete_id.set(None)>
                        <div class="routines-confirm-dialog" on:click=move |e: web_sys::MouseEvent| e.stop_propagation()>
                            <div class="routines-confirm-title">"Delete Routine"</div>
                            <div class="routines-confirm-msg">
                                "Are you sure you want to delete \""
                                {del_name}
                                "\"? This action cannot be undone."
                            </div>
                            <div class="routines-confirm-actions">
                                <button
                                    class="routines-btn routines-btn--muted"
                                    on:click=move |_| set_confirm_delete_id.set(None)
                                >
                                    "Cancel"
                                </button>
                                <button
                                    class="routines-btn routines-btn--danger"
                                    on:click=move |_| handle_delete.run(del_id2.clone())
                                >
                                    <IconTrash2 size=13 />
                                    " Delete"
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}
        </ModalOverlay>
    }
}
