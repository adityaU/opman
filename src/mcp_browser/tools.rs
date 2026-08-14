//! Tool schemas and dispatch for the browser MCP server.
//!
//! Every tool but `browser_screenshot` answers in text, and the text is the compact
//! outline rather than markup. That is the whole point of the server: a model can open a
//! page, read it, click something, and read the result for roughly the token cost of a
//! short file, however heavy the site is.

use serde_json::{json, Value};

use super::{Internal, Project};

/// Ceiling on a page-text read, mirrored from the backend default so the schema does not
/// promise more than the page script will return.
const TEXT_LIMIT_HINT: usize = 8_000;

pub(super) fn definitions() -> Value {
    // Optional everywhere: the server already knows the project it was launched for, and
    // that project's browser is the one the user is watching. Naming another is the rare
    // case, not the default one.
    let project = json!({
        "type": "string",
        "description": "Absolute path of the project whose browser to act on. Omit to use your own project — that is almost always what you want.",
    });
    let reference = json!({
        "type": "string",
        "description": "A [ref=eN] handle from the most recent browser_snapshot. Refs are invalidated by the next snapshot — always act on the newest one.",
    });

    json!({
        "tools": [
            {
                "name": "browser_open",
                "description": "Open a URL in your project's browser and return the page as a compact outline. The user's workspace reveals that browser, so they watch the same page you are working on.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "url": { "type": "string", "description": "URL to open. A bare host gets https://." },
                    },
                    "required": ["url"],
                },
            },
            {
                "name": "browser_snapshot",
                "description": "Read the page as an indented outline of its interactive and structural elements, each actionable one tagged [ref=eN]. This is the primary way to see a page — it costs a fraction of the HTML and is what click/type take their targets from. Never ask for raw HTML instead.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "max_nodes": { "type": "integer", "description": "Cap on outline lines (default 400). Lower it on huge pages." },
                        "viewport_only": { "type": "boolean", "description": "Only what is currently on screen. Default false." },
                    },
                },
            },
            {
                "name": "browser_read_text",
                "description": "Extract the main readable text of the page — article body, docs content — with navigation and boilerplate dropped. Use this when you need to READ a page rather than act on it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "max_chars": {
                            "type": "integer",
                            "description": format!("Truncation limit (default {TEXT_LIMIT_HINT})."),
                        },
                    },
                },
            },
            {
                "name": "browser_click",
                "description": "Click an element by its [ref=eN] handle, then return the resulting page outline. The click is dispatched as a real mouse event at the element's position.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project.clone(), "ref": reference },
                    "required": ["ref"],
                },
            },
            {
                "name": "browser_type",
                "description": "Focus a field by [ref=eN], replace its contents with text, optionally press Enter, and return the resulting outline.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "ref": reference,
                        "text": { "type": "string" },
                        "submit": { "type": "boolean", "description": "Press Enter after typing. Default false." },
                    },
                    "required": ["ref", "text"],
                },
            },
            {
                "name": "browser_press_key",
                "description": "Press a key or chord on the page, e.g. Enter, Escape, Tab, ArrowDown, Control+a.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project.clone(), "key": { "type": "string" } },
                    "required": ["key"],
                },
            },
            {
                "name": "browser_scroll",
                "description": "Scroll the page vertically. Positive delta scrolls down. Prefer a full snapshot over scroll-and-peek unless the page is very long.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "delta_y": { "type": "integer", "description": "Pixels to scroll; positive is down." },
                    },
                    "required": ["delta_y"],
                },
            },
            {
                "name": "browser_navigate",
                "description": "Go back, forward, or reload in the browser's history.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "direction": { "type": "string", "enum": ["back", "forward", "reload"] },
                    },
                    "required": ["direction"],
                },
            },
            {
                "name": "browser_screenshot",
                "description": "A JPEG of the current viewport. Expensive in tokens compared to a snapshot — use it only when layout, rendering, or visual state is the actual question.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project.clone(),
                        "quality": { "type": "integer", "description": "JPEG quality 1-100, default 60." },
                    },
                },
            },
            {
                "name": "browser_list_panes",
                "description": "List the browsers that are open, one per project, with the URL and title of each. You rarely need this — every other tool defaults to your own project's browser.",
                "inputSchema": { "type": "object", "properties": {} },
            },
        ],
    })
}

/// Translate one tool call into an `/internal/browser` operation.
///
/// The mapping is deliberately total: an unrecognised name produces a message the model
/// can act on rather than a silent empty result.
pub(super) async fn dispatch_tool(
    internal: Option<&Internal>,
    project: &Project,
    params: Option<Value>,
) -> String {
    let Some(internal) = internal else {
        return "The browser API is unavailable — the opman web server is not running.".into();
    };
    let params = params.unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let Some(mut operation) = to_operation(name, &args) else {
        return format!("Unknown tool `{name}`.");
    };
    // The agent's own project unless it named another. Injected here rather than left to
    // the schema so an agent cannot reach another project's browser by omission.
    if let Some(object) = operation.as_object_mut() {
        let target = args
            .get("project")
            .and_then(Value::as_str)
            .unwrap_or_else(|| project.as_str());
        object.insert("project".into(), json!(target));
    }
    match super::post(internal, operation).await {
        Ok(value) => render(name, &value),
        Err(e) => format!("{name} failed: {e}"),
    }
}

/// Build the tagged operation body. Returns `None` for an unknown tool.
fn to_operation(name: &str, args: &Value) -> Option<Value> {
    let mut body = match name {
        "browser_open" => json!({ "op": "open", "url": args.get("url") }),
        "browser_snapshot" => json!({
            "op": "snapshot",
            "max_nodes": args.get("max_nodes"),
            "viewport_only": args.get("viewport_only"),
        }),
        "browser_read_text" => json!({ "op": "text", "max_chars": args.get("max_chars") }),
        "browser_click" => json!({ "op": "click", "ref": args.get("ref") }),
        "browser_type" => json!({
            "op": "type",
            "ref": args.get("ref"),
            "text": args.get("text"),
            "submit": args.get("submit"),
        }),
        "browser_press_key" => json!({ "op": "key", "key": args.get("key") }),
        "browser_scroll" => json!({ "op": "scroll", "delta_y": args.get("delta_y") }),
        "browser_navigate" => {
            let direction = args
                .get("direction")
                .and_then(Value::as_str)
                .unwrap_or("reload");
            json!({ "op": direction })
        }
        "browser_screenshot" => json!({ "op": "screenshot", "quality": args.get("quality") }),
        "browser_list_panes" => return Some(json!({ "op": "list" })),
        _ => return None,
    };

    if let Some(object) = body.as_object_mut() {
        // A `null` from a missing optional argument would fail the typed deserialiser on
        // the server; dropping the key lets the field default instead.
        object.retain(|_, value| !value.is_null());
    }
    Some(body)
}

/// Turn a response into the text the model reads. Outlines are returned as-is with a one
/// line header, because indentation is the structure — wrapping them in JSON would cost
/// tokens and hide it.
fn render(name: &str, value: &Value) -> String {
    let field = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default();

    if name == "browser_screenshot" {
        return format!(
            "Screenshot of {} (base64 JPEG):\n{}",
            field("url"),
            field("data")
        );
    }
    if name == "browser_list_panes" {
        let panes = value.get("panes").and_then(Value::as_array);
        return match panes.filter(|list| !list.is_empty()) {
            None => "No browsers are open. Call browser_open with a URL — one is created for your project.".into(),
            Some(list) => list
                .iter()
                .map(|pane| {
                    format!(
                        "{}  {}  {}",
                        pane.get("project").and_then(Value::as_str).unwrap_or("?"),
                        pane.get("title").and_then(Value::as_str).unwrap_or(""),
                        pane.get("url").and_then(Value::as_str).unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
    }
    if name == "browser_read_text" {
        let truncated = value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or_default();
        let suffix = if truncated {
            "\n\n[truncated — raise max_chars for more]"
        } else {
            ""
        };
        return format!("{}\n{}\n\n{}{suffix}", field("title"), field("url"), field("text"));
    }

    let outline = field("outline");
    if outline.is_empty() {
        return format!("{}\n{}", field("title"), field("url"));
    }
    let truncated = value
        .get("truncated")
        .and_then(Value::as_bool)
        .unwrap_or_default();
    let note = if truncated {
        "\n[outline truncated — raise max_nodes, or scroll and re-snapshot]"
    } else {
        ""
    };
    format!(
        "{}\n{}\n\n{outline}{note}",
        field("title"),
        field("url")
    )
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tools_tests;
