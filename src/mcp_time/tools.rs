//! Tool implementations for the MCP time server.

use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;

// ─── time_now ────────────────────────────────────────────────────────────────

pub(super) fn tool_time_now(args: &serde_json::Value) -> String {
    let system_tz_name = system_timezone_name();
    let now_local: DateTime<Local> = Local::now();

    match args.get("timezone").and_then(|v| v.as_str()) {
        None | Some("") | Some("local") => {
            // Just system time
            format!(
                "Current time: {}\nTimezone: {} (system default)",
                now_local.format("%Y-%m-%d %H:%M:%S %Z (%:z)"),
                system_tz_name
            )
        }
        Some(tz_str) => {
            match tz_str.parse::<Tz>() {
                Ok(tz) => {
                    let now_in_tz: DateTime<Tz> = Utc::now().with_timezone(&tz);
                    format!(
                        "Current time in {}: {}\nSystem timezone: {} ({})",
                        tz_str,
                        now_in_tz.format("%Y-%m-%d %H:%M:%S %Z (%:z)"),
                        system_tz_name,
                        now_local.format("%Y-%m-%d %H:%M:%S %Z (%:z)")
                    )
                }
                Err(_) => format!(
                    "Unknown timezone: \"{}\". Use time_zones tool to search for valid IANA names.\nSystem time: {} ({})",
                    tz_str,
                    now_local.format("%Y-%m-%d %H:%M:%S %Z (%:z)"),
                    system_tz_name
                ),
            }
        }
    }
}

// ─── time_convert ────────────────────────────────────────────────────────────

pub(super) fn tool_time_convert(args: &serde_json::Value) -> String {
    let datetime_str = match args.get("datetime").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return "Missing required argument: 'datetime'".into(),
    };
    let from_str = match args.get("from_timezone").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return "Missing required argument: 'from_timezone'".into(),
    };
    let to_str = match args.get("to_timezone").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return "Missing required argument: 'to_timezone'".into(),
    };

    // Resolve "local" to the system timezone name
    let system_tz = system_timezone_name();
    let from_str = if from_str == "local" {
        system_tz.as_str()
    } else {
        from_str
    };
    let to_str = if to_str == "local" {
        system_tz.as_str()
    } else {
        to_str
    };

    // Parse source timezone
    let from_tz: Tz = match from_str.parse() {
        Ok(tz) => tz,
        Err(_) => {
            return format!(
                "Unknown source timezone: \"{}\". Use time_zones tool to search.",
                from_str
            )
        }
    };

    // Parse target timezone
    let to_tz: Tz = match to_str.parse() {
        Ok(tz) => tz,
        Err(_) => {
            return format!(
                "Unknown target timezone: \"{}\". Use time_zones tool to search.",
                to_str
            )
        }
    };

    // Parse the datetime string (try multiple formats)
    let formats = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];

    let naive_dt = formats
        .iter()
        .find_map(|fmt| chrono::NaiveDateTime::parse_from_str(datetime_str, fmt).ok());

    let naive_dt = match naive_dt {
        Some(dt) => dt,
        None => return format!(
            "Could not parse datetime: \"{}\". Use format: \"YYYY-MM-DD HH:MM:SS\" or \"YYYY-MM-DD HH:MM\".",
            datetime_str
        ),
    };

    // Localize to source timezone
    let from_dt: DateTime<Tz> = match from_tz.from_local_datetime(&naive_dt).single() {
        Some(dt) => dt,
        None => {
            return format!(
                "Ambiguous or invalid local time \"{}\" in timezone \"{}\" (e.g. DST transition).",
                datetime_str, from_str
            )
        }
    };

    // Convert to target timezone
    let to_dt: DateTime<Tz> = from_dt.with_timezone(&to_tz);

    format!(
        "{} {} → {} {}",
        from_dt.format("%Y-%m-%d %H:%M:%S %Z"),
        from_str,
        to_dt.format("%Y-%m-%d %H:%M:%S %Z"),
        to_str,
    )
}

// ─── time_zones ──────────────────────────────────────────────────────────────

pub(super) fn tool_time_zones(args: &serde_json::Value) -> String {
    let search = args
        .get("search")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let all: Vec<&str> = chrono_tz::TZ_VARIANTS
        .iter()
        .map(|tz| tz.name())
        .filter(|name| search.is_empty() || name.to_lowercase().contains(&search))
        .collect();

    if all.is_empty() {
        return format!("No timezones found matching \"{}\".", search);
    }

    format!(
        "{} timezone(s){}:\n{}",
        all.len(),
        if search.is_empty() {
            String::new()
        } else {
            format!(" matching \"{}\"", search)
        },
        all.join("\n")
    )
}

// ─── System timezone helper ──────────────────────────────────────────────────

fn system_timezone_name() -> String {
    // Try reading /etc/localtime symlink → extract IANA name from path
    if let Ok(target) = std::fs::read_link("/etc/localtime") {
        let path_str = target.to_string_lossy();
        // Typical: /var/db/timezone/zoneinfo/America/New_York  or
        //          /usr/share/zoneinfo/America/New_York
        if let Some(pos) = path_str.find("zoneinfo/") {
            let tz_name = &path_str[pos + "zoneinfo/".len()..];
            if !tz_name.is_empty() {
                return tz_name.to_string();
            }
        }
    }

    // Fallback: use the offset from chrono Local
    let now = Local::now();
    let offset = now.offset().local_minus_utc();
    let h = offset / 3600;
    let m = (offset.abs() % 3600) / 60;
    if m == 0 {
        format!("UTC{:+}", h)
    } else {
        format!("UTC{:+}:{:02}", h, m)
    }
}
