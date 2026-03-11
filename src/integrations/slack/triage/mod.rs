//! AI triage logic for routing Slack messages to the correct project/session.

mod prompt;

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::warn;

pub use prompt::build_triage_prompt;

/// Return the triage project directory path, checking `dirs::config_dir()/opman/slack/`
/// and `~/.config/opman/slack/`, following symlinks transparently.
pub fn triage_project_dir() -> Result<PathBuf> {
    let subpath = std::path::Path::new("opman").join("slack");

    // 1. Primary: dirs::config_dir()/opman/slack/
    if let Some(primary) = dirs::config_dir() {
        let p = primary.join(&subpath);
        if std::fs::metadata(&p).is_ok() {
            return Ok(p);
        }
    }

    // 2. Fallback: ~/.config/opman/slack/
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join(&subpath);
        if std::fs::metadata(&p).is_ok() {
            return Ok(p);
        }
    }

    // 3. Neither exists – return the primary path (callers will create it).
    let dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("opman")
        .join("slack");
    Ok(dir)
}

/// Parse the triage AI response to extract routing info, direct answer, or session creation request.
/// Returns (project_path, model, rewritten_query, direct_answer, create_session, connect_only, error).
pub fn parse_triage_response(
    response_text: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
    bool,
    Option<String>,
) {
    // Try to extract JSON from the response text.
    // The AI may return raw JSON, or wrap it in markdown code fences.
    let json_str = extract_json(response_text);

    if let Some(json_str) = json_str {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
            let action = val.get("action").and_then(|v| v.as_str()).unwrap_or("");

            if action == "direct_answer" {
                let direct_answer = val
                    .get("direct_answer")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                return (None, None, None, direct_answer, false, false, None);
            }

            let is_create_session = action == "create_session";
            let is_connect_only = action == "connect";

            let project_path = val
                .get("project_path")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let model = val
                .get("model")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "null")
                .map(|s| s.to_string());
            let rewritten_query = if is_connect_only {
                // Connect-only actions never have a query to forward.
                None
            } else {
                val.get("rewritten_query")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "null")
                    .map(|s| s.to_string())
            };
            let error = val
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            return (
                project_path,
                model,
                rewritten_query,
                None,
                is_create_session,
                is_connect_only,
                error,
            );
        }
    }

    // JSON parsing failed entirely.
    warn!(
        "Triage response was not valid JSON (first 200 chars): {}",
        response_text.chars().take(200).collect::<String>()
    );
    (
        None,
        None,
        None,
        None,
        false,
        false,
        Some("Could not parse triage response. Please specify the project explicitly.".to_string()),
    )
}

/// Extract JSON from a response that may contain markdown code fences or surrounding text.
///
/// Handles these common patterns:
/// - Raw JSON: `{"action": "route", ...}`
/// - Fenced JSON: ` ```json\n{"action": "route", ...}\n``` `
/// - Fenced (no lang): ` ```\n{"action": "route", ...}\n``` `
/// - JSON embedded in prose: `Here's my answer:\n{"action": ...}\nHope that helps!`
fn extract_json(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // 1. Try raw JSON first.
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }

    // 2. Try extracting from markdown code fences.
    // Match ```json ... ``` or ``` ... ```
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        // Skip optional language tag (e.g., "json")
        let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_fence[content_start..];
        if let Some(end) = content.find("```") {
            let inner = content[..end].trim();
            if inner.starts_with('{') && inner.ends_with('}') {
                return Some(inner.to_string());
            }
        }
    }

    // 3. Try finding the first JSON object in the text.
    if let Some(start) = trimmed.find('{') {
        // Find the matching closing brace by counting braces.
        let bytes = trimmed.as_bytes();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape = false;
        for (i, &b) in bytes[start..].iter().enumerate() {
            if escape {
                escape = false;
                continue;
            }
            match b {
                b'\\' if in_string => escape = true,
                b'"' => in_string = !in_string,
                b'{' if !in_string => depth += 1,
                b'}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        let candidate = &trimmed[start..start + i + 1];
                        // Validate it's actually parseable JSON.
                        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                            return Some(candidate.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    None
}
