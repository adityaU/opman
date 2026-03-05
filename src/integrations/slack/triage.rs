//! AI triage logic for routing Slack messages to the correct project/session.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::warn;

use super::types::SessionMeta;

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

/// Build the system prompt for the triage AI session.
/// Includes the list of known projects and their sessions so the AI can
/// either route to a project or answer informational queries directly.
pub fn build_triage_prompt(
    projects: &[(String, String)],
    sessions: &[SessionMeta],
    user_text: &str,
) -> String {
    let project_list: String = projects
        .iter()
        .enumerate()
        .map(|(i, (name, path))| format!("  {}. \"{}\" — {}", i + 1, name, path))
        .collect::<Vec<_>>()
        .join("\n");

    // Build a concise session summary grouped by project.
    let session_summary: String = if sessions.is_empty() {
        "  (no sessions)".to_string()
    } else {
        // Group sessions by project name.
        let mut by_project: std::collections::BTreeMap<&str, Vec<&SessionMeta>> =
            std::collections::BTreeMap::new();
        for s in sessions.iter().filter(|s| s.parent_id.is_empty()) {
            by_project.entry(&s.project_name).or_default().push(s);
        }
        let mut lines = Vec::new();
        for (pname, mut slist) in by_project {
            slist.sort_by(|a, b| b.updated.cmp(&a.updated));
            lines.push(format!(
                "  Project \"{}\" ({} sessions):",
                pname,
                slist.len()
            ));
            for s in slist.iter().take(10) {
                let short_id = &s.id[..8.min(s.id.len())];
                let title = if s.title.is_empty() {
                    "(untitled)".to_string()
                } else {
                    s.title.clone()
                };
                let updated = if s.updated > 0 {
                    chrono::DateTime::from_timestamp(s.updated as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    "unknown".to_string()
                };
                lines.push(format!(
                    "    - ID: {} | Title: \"{}\" | Updated: {}",
                    short_id, title, updated
                ));
            }
            if slist.len() > 10 {
                lines.push(format!("    ... and {} more", slist.len() - 10));
            }
        }
        lines.join("\n")
    };

    format!(
        r#"You are a smart triage assistant for a software development environment manager called opman.

The user sent a message via Slack. You have FOUR possible actions:

**Action A — Direct Answer**: If the user's message is an INFORMATIONAL QUERY that you can answer from the project/session data below, answer it directly. Examples:
- "list all sessions in X" → answer with the session list
- "what projects are configured?" → answer with the project list
- "how many sessions does X have?" → answer with the count
- "what's the status of X?" → answer with project/session info
- "show me recent sessions" → answer with recent session data

**Action B — Route to Project**: If the user's message is a CODING TASK or QUESTION that requires a coding assistant (e.g., "fix the bug in X", "add a feature to Y", "explain the code in Z", "in session X do Y", "ask opencode to do Z"), route it to the appropriate project. The rewritten_query should contain ONLY the substantive task/question to send to the coding assistant.

**Action C — Create New Session**: If the user EXPLICITLY requests creating a new session, use this action. The user must show clear intent — look for phrases like:
- "create a new session in X"
- "start a fresh session for X"
- "new session for project X"
- "spin up a session in X"
- "open a new session"
Do NOT use Action C unless the user explicitly asks for session creation. Regular coding tasks should use Action B even if all sessions happen to be busy.

**Action D — Connect to Session**: If the user's message is PURELY about connecting, attaching, or switching to a session or project, with NO coding task or question to forward. This just links the Slack thread to the session without sending any message. Examples:
- "connect to ses_34bb"
- "attach to the opencode session"
- "switch to project X"
- "connect me to the opencode-manager project"
- "go to session X"
- "use session Y"
This is different from Action B. Action B is for when the user has an actual task/question to send. Action D is for when the user only wants to establish a connection.

Available projects:
{project_list}

Session data:
{session_summary}

User's message:
"{user_text}"

Respond with EXACTLY this JSON format (no markdown, no explanation):

For Action A (direct answer):
{{"action": "direct_answer", "direct_answer": "<your answer to the user's informational query>"}}

For Action B (route to project with a task):
{{"action": "route", "project_name": "<name>", "project_path": "<path>", "model": "<model or null>", "rewritten_query": "<the user's actual task/question with ALL routing metadata removed>", "confidence": <0.0-1.0>}}

For Action C (create new session):
{{"action": "create_session", "project_name": "<name>", "project_path": "<path>", "model": "<model or null>", "rewritten_query": "<the user's actual task/question with the session-creation request removed, or null if there's no follow-up task>", "confidence": <0.0-1.0>}}

For Action D (connect only, no task to forward):
{{"action": "connect", "project_name": "<name>", "project_path": "<path>", "model": null, "rewritten_query": null, "confidence": <0.0-1.0>}}

If you cannot determine the project with reasonable confidence (>0.5) for a routing or session-creation request:
{{"action": "route", "project_name": null, "project_path": null, "model": null, "rewritten_query": null, "confidence": 0.0, "error": "Could not determine which project you mean. Please specify the project name."}}

Rules:
- PREFER Action A for any query that can be answered from the project/session data above.
- Use Action B when the user has a real coding task or question to send to a session (e.g., "in session X, do this", "ask opencode to refactor the function", "fix the bug in Y").
- Use Action D when the user ONLY wants to connect/attach/switch to a session or project WITHOUT any coding task. The key distinction: if there is NO substantive work to forward, use Action D. If there IS work to forward, use Action B.
- Only use Action C when the user EXPLICITLY requests creating a new session. Do NOT infer session creation from a coding task.
- For Action A, format your direct_answer clearly with bullet points, counts, and relevant details. Use Slack mrkdwn formatting (*bold*, `code`, etc.).
- For Action B, C, and D, match project names loosely (abbreviations, partial names are OK).
- If the message mentions a file path, match the project whose path is a prefix.
- If only one project exists, assume it's the target for Action B/C/D unless the message explicitly says otherwise.
- For model detection in Action B/C, look for keywords like "claude", "sonnet", "opus", "gpt", "o1", "gemini", etc.
- CRITICAL — rewritten_query rules (Action B and C):
  - Remove ALL project names, project paths, session names, session IDs, model preferences, and routing instructions.
  - Strip phrases like "in the X project", "send this to Y", "direct this to Z", "using model W", "in session ABC", "@session", "@list-sessions", "create a new session", "start fresh session", etc.
  - The rewritten_query must read as a clean, standalone message — as if the user typed it directly to a coding assistant with no routing context.
  - Keep ONLY the substantive task, question, or instruction.
  - If the entire message is just a session creation request with no real task, set rewritten_query to null.
  - If the entire message is just routing/connecting with no real task, set rewritten_query to null (and use Action D).
"#,
        project_list = project_list,
        session_summary = session_summary,
        user_text = user_text,
    )
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
