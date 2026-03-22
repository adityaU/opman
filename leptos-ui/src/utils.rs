//! Shared utility functions for the Leptos UI.

/// Normalize an epoch timestamp to milliseconds.
///
/// The upstream opencode server returns `Date.now()` (milliseconds), but
/// some code paths may pass seconds.  Heuristic: values above 10 billion
/// are already milliseconds; smaller values are seconds.
#[inline]
fn to_ms(ts: f64) -> f64 {
    if ts > 10_000_000_000.0 {
        ts
    } else {
        ts * 1000.0
    }
}

/// Format an epoch timestamp (seconds **or** milliseconds) as a short
/// relative string: "now", "5m ago", "2h ago", "3d ago", or "Mar 21".
pub fn format_relative_time(ts: f64) -> String {
    if ts <= 0.0 {
        return String::new();
    }
    let now_ms = js_sys::Date::now();
    let then_ms = to_ms(ts);
    let diff_ms = now_ms - then_ms;

    // Future or just happened → "now"
    if diff_ms < 60_000.0 {
        return "now".to_string();
    }
    let diff_min = (diff_ms / 60_000.0).floor() as i64;
    if diff_min < 60 {
        return format!("{}m ago", diff_min);
    }
    let diff_hrs = diff_min / 60;
    if diff_hrs < 24 {
        return format!("{}h ago", diff_hrs);
    }
    let diff_days = diff_hrs / 24;
    if diff_days < 7 {
        return format!("{}d ago", diff_days);
    }
    // Fallback: short date like "Mar 21"
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(then_ms));
    let month = d.get_month() as usize; // 0-indexed
    let day = d.get_date();
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let m = months.get(month).unwrap_or(&"???");
    format!("{} {}", m, day)
}

/// Format an epoch timestamp as absolute clock time "HH:MM".
pub fn format_clock_time(ts: f64) -> String {
    if ts == 0.0 {
        return String::new();
    }
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(to_ms(ts)));
    let hours = d.get_hours();
    let mins = d.get_minutes();
    format!("{:02}:{:02}", hours, mins)
}
