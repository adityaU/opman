/// MCP time server — runs as `opman --mcp-time`
///
/// Exposes three tools to the AI:
///   - `time_now`       — current time in the system timezone (or a given zone)
///   - `time_convert`   — convert a datetime from one timezone to another
///   - `time_zones`     — list/search IANA timezone names
///
/// The server speaks JSON-RPC 2.0 over stdin/stdout (standard MCP stdio transport).
/// No Unix socket is needed — all operations are pure computation.
use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// ─── JSON-RPC types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
    id: serde_json::Value,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub async fn run_mcp_time_bridge() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: McpRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) },
                    "id": null
                });
                write_response(&mut stdout, &resp).await?;
                continue;
            }
        };

        let resp = match req.method.as_str() {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "opman-time",
                        "version": "1.0.0"
                    }
                },
                "id": req.id
            }),

            "notifications/initialized" => continue,

            "tools/list" => serde_json::json!({
                "jsonrpc": "2.0",
                "result": { "tools": tool_definitions() },
                "id": req.id
            }),

            "tools/call" => {
                let result = dispatch_tool(req.params);
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": { "content": result },
                    "id": req.id
                })
            }

            other => serde_json::json!({
                "jsonrpc": "2.0",
                "error": { "code": -32601, "message": format!("Method not found: {}", other) },
                "id": req.id
            }),
        };

        write_response(&mut stdout, &resp).await?;
    }

    Ok(())
}

async fn write_response(
    stdout: &mut tokio::io::Stdout,
    resp: &serde_json::Value,
) -> anyhow::Result<()> {
    stdout.write_all(serde_json::to_string(resp)?.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

// ─── Tool definitions ────────────────────────────────────────────────────────

fn tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "time_now",
            "description": "Get the current date and time. Returns the time in the system's local timezone by default. Optionally specify an IANA timezone name (e.g. \"America/New_York\", \"Europe/London\", \"Asia/Tokyo\") to get the current time in that zone. Also always reports the system's default timezone.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timezone": {
                        "type": "string",
                        "description": "Optional IANA timezone name (e.g. \"America/New_York\", \"UTC\", \"Asia/Kolkata\"). Defaults to the system local timezone."
                    }
                }
            }
        },
        {
            "name": "time_convert",
            "description": "Convert a date/time from one timezone to another. Accepts a datetime string and source/target IANA timezone names.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datetime": {
                        "type": "string",
                        "description": "The datetime to convert. Accepted formats: \"2024-01-15 14:30:00\", \"2024-01-15T14:30:00\", \"2024-01-15 14:30\" (seconds optional). Time is interpreted in the `from_timezone`."
                    },
                    "from_timezone": {
                        "type": "string",
                        "description": "IANA source timezone name (e.g. \"America/Los_Angeles\"). Use \"local\" for the system timezone."
                    },
                    "to_timezone": {
                        "type": "string",
                        "description": "IANA target timezone name (e.g. \"Asia/Kolkata\"). Use \"local\" for the system timezone."
                    }
                },
                "required": ["datetime", "from_timezone", "to_timezone"]
            }
        },
        {
            "name": "time_zones",
            "description": "List or search IANA timezone names. Returns all timezones or filters by a search term. Useful for discovering valid timezone strings to pass to other time tools.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": {
                        "type": "string",
                        "description": "Optional case-insensitive search term to filter timezone names (e.g. \"india\", \"pacific\", \"europe\")."
                    }
                }
            }
        }
    ])
}

// ─── Tool dispatch ───────────────────────────────────────────────────────────

fn dispatch_tool(params: Option<serde_json::Value>) -> serde_json::Value {
    let params = params.unwrap_or(serde_json::json!({}));
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

    let text = match tool_name {
        "time_now" => tool_time_now(&args),
        "time_convert" => tool_time_convert(&args),
        "time_zones" => tool_time_zones(&args),
        other => format!("Unknown tool: {}", other),
    };

    serde_json::json!([{ "type": "text", "text": text }])
}

// ─── time_now ────────────────────────────────────────────────────────────────

fn tool_time_now(args: &serde_json::Value) -> String {
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

fn tool_time_convert(args: &serde_json::Value) -> String {
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
    let from_str = if from_str == "local" { system_tz.as_str() } else { from_str };
    let to_str = if to_str == "local" { system_tz.as_str() } else { to_str };

    // Parse source timezone
    let from_tz: Tz = match from_str.parse() {
        Ok(tz) => tz,
        Err(_) => return format!("Unknown source timezone: \"{}\". Use time_zones tool to search.", from_str),
    };

    // Parse target timezone
    let to_tz: Tz = match to_str.parse() {
        Ok(tz) => tz,
        Err(_) => return format!("Unknown target timezone: \"{}\". Use time_zones tool to search.", to_str),
    };

    // Parse the datetime string (try multiple formats)
    let formats = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];

    let naive_dt = formats.iter().find_map(|fmt| {
        chrono::NaiveDateTime::parse_from_str(datetime_str, fmt).ok()
    });

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
        None => return format!(
            "Ambiguous or invalid local time \"{}\" in timezone \"{}\" (e.g. DST transition).",
            datetime_str, from_str
        ),
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

fn tool_time_zones(args: &serde_json::Value) -> String {
    let search = args.get("search")
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
        if search.is_empty() { String::new() } else { format!(" matching \"{}\"", search) },
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
