//! File edits and cross-session search handlers.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use crate::app::base_url;

/// GET /api/session/{session_id}/file-edits
///
/// Returns all file edits tracked for a session (original and new content).
pub async fn get_file_edits(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let edits = state.web_state.get_file_edits(&session_id).await;

    // Deduplicate: only keep the latest edit per file path
    let mut latest_by_path: std::collections::HashMap<String, &super::super::web_state::FileEditRecord> =
        std::collections::HashMap::new();
    for edit in &edits {
        latest_by_path.insert(edit.path.clone(), edit);
    }

    let mut result: Vec<FileEditEntry> = latest_by_path
        .into_values()
        .map(|e| FileEditEntry {
            path: e.path.clone(),
            original_content: e.original_content.clone(),
            new_content: e.new_content.clone(),
            timestamp: e.timestamp.clone(),
            index: e.index,
        })
        .collect();
    result.sort_by_key(|e| e.index);

    let file_count = result.len();

    Ok(Json(FileEditsResponse {
        session_id,
        edits: result,
        file_count,
    }))
}

// ── Cross-Session Search ────────────────────────────────────────────

/// Query parameters for the search endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

pub(super) fn default_search_limit() -> usize {
    50
}

/// GET /api/project/{idx}/search?q=<query>&limit=50
///
/// Searches across all sessions in a project by fetching messages from the
/// opencode API and doing case-insensitive substring matching on message text,
/// tool call names, arguments, and output.
pub async fn search_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(project_idx): axum::extract::Path<usize>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> WebResult<impl IntoResponse> {
    let query = params.q.trim().to_string();
    if query.is_empty() {
        return Ok(Json(SearchResponse {
            query,
            results: vec![],
            total: 0,
        }));
    }

    let (project_path, project_name, sessions) = state
        .web_state
        .get_project_sessions(project_idx)
        .await
        .ok_or(WebError::BadRequest("Invalid project index".into()))?;

    let base = base_url().to_string();
    let dir = project_path.to_string_lossy().to_string();
    let query_lower = query.to_lowercase();
    let limit = params.limit.min(200); // cap at 200

    let mut results: Vec<SearchResultEntry> = Vec::new();

    // Search each session's messages
    for (session_id, session_title) in &sessions {
        if results.len() >= limit {
            break;
        }

        // Fetch messages for this session
        let resp = match state
            .http_client
            .get(format!("{}/session/{}/message", base, session_id))
            .header("x-opencode-directory", &dir)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Normalise into flat Vec
        let messages: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
            arr.iter().collect()
        } else if let Some(obj) = body.as_object() {
            obj.values().collect()
        } else {
            continue;
        };

        for msg in &messages {
            if results.len() >= limit {
                break;
            }

            let role = msg
                .pointer("/info/role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let msg_id = msg
                .pointer("/info/id")
                .or_else(|| msg.pointer("/info/messageID"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let timestamp = msg
                .pointer("/info/time/created")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            // Collect all searchable text from parts
            let parts = msg.get("parts").and_then(|v| v.as_array());
            if let Some(parts) = parts {
                for part in parts {
                    let mut searchable_texts: Vec<&str> = Vec::new();

                    // Text content
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        searchable_texts.push(text);
                    }

                    // Tool call name
                    if let Some(name) = part.get("toolName").and_then(|v| v.as_str()) {
                        searchable_texts.push(name);
                    }

                    // Tool call args (stringify)
                    if let Some(args) = part.get("args") {
                        if let Some(s) = args.as_str() {
                            searchable_texts.push(s);
                        }
                    }

                    // Tool call output/result
                    if let Some(output) = part.get("output").and_then(|v| v.as_str()) {
                        searchable_texts.push(output);
                    }
                    if let Some(result) = part.get("result").and_then(|v| v.as_str()) {
                        searchable_texts.push(result);
                    }

                    // Check if any text matches
                    for text in &searchable_texts {
                        if text.to_lowercase().contains(&query_lower) {
                            // Build snippet: find match position and extract context
                            let snippet = build_snippet(text, &query_lower, 120);
                            results.push(SearchResultEntry {
                                session_id: session_id.clone(),
                                session_title: session_title.clone(),
                                project_name: project_name.clone(),
                                message_id: msg_id.clone(),
                                role: role.to_string(),
                                snippet,
                                timestamp,
                            });
                            break; // one match per message is enough
                        }
                    }

                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    let total = results.len();
    Ok(Json(SearchResponse {
        query,
        results,
        total,
    }))
}

/// Build a snippet around the first occurrence of `needle` in `haystack`.
/// Returns at most `max_len` characters with "..." ellipsis if truncated.
pub(super) fn build_snippet(haystack: &str, needle_lower: &str, max_len: usize) -> String {
    let lower = haystack.to_lowercase();
    let pos = match lower.find(needle_lower) {
        Some(p) => p,
        None => return haystack.chars().take(max_len).collect(),
    };

    // Compute a window around the match
    let context = max_len / 2;
    let start = pos.saturating_sub(context);
    let end = (pos + needle_lower.len() + context).min(haystack.len());

    // Adjust to char boundaries
    let start = haystack
        .char_indices()
        .find(|(i, _)| *i >= start)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = haystack
        .char_indices()
        .rev()
        .find(|(i, _)| *i <= end)
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(haystack.len());

    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(&haystack[start..end]);
    if end < haystack.len() {
        snippet.push_str("...");
    }
    snippet
}
