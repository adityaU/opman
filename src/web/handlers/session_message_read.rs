//! Session message reading and pagination.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::handlers::common::{default_base_url, resolve_project_dir};
use crate::web::types::*;

/// Query parameters for paginated message fetching.
#[derive(serde::Deserialize)]
pub struct MessagePageQuery {
    /// Maximum number of messages to return. Omit or 0 for all.
    pub limit: Option<usize>,
    /// Only return messages created **before** this Unix-ms timestamp (exclusive).
    /// Used for "load older" pagination — pass the oldest timestamp from the
    /// previous page to fetch the preceding chunk.
    pub before: Option<u64>,
}

/// Selects which runner answers a directory-scoped catalog request (models, commands).
#[derive(serde::Deserialize, Default)]
pub struct RunnerQuery {
    /// Runtime runner whose catalog should be returned. Omitted means the default engine.
    pub runner: Option<String>,
}

/// GET /api/session/:id/messages — fetch messages for a session.
///
/// Supports optional pagination via query parameters:
///   - `?limit=N`             — return only the N most recent messages
///   - `?before=TIMESTAMP`    — return messages before this Unix-ms timestamp
///   - `?limit=N&before=T`    — load N messages before timestamp T
///
/// Response: `{ "messages": [...], "has_more": bool, "total": usize }`
/// Messages are sorted by creation time (ascending — oldest first within the page).
pub async fn get_session_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Query(page): Query<MessagePageQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
    {
        let body = state
            .runner_registry
            .messages(&session_id, &dir)
            .await
            .map_err(WebError::runner)?;
        return Ok(Json(paginate_messages(
            body,
            page.limit.unwrap_or(0),
            page.before,
        )));
    }
    let base = default_base_url(&state).await?;
    let resp = state
        .http_client
        .get(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    let limit = page.limit.unwrap_or(0);
    Ok(Json(paginate_messages(body, limit, page.before)))
}

/// Normalise a raw upstream messages payload into a flat, chronologically
/// sorted list and apply optional `limit` / `before` pagination.
///
/// Upstream may return either an array of messages or an object keyed by
/// message ID. Returns `{ messages, has_more, total }`.
pub(crate) fn paginate_messages(
    body: serde_json::Value,
    limit: usize,
    before: Option<u64>,
) -> serde_json::Value {
    // Normalise the response into a flat Vec — upstream may return an array
    // or an object keyed by message ID.
    let mut all_messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.clone()
    } else if let Some(obj) = body.as_object() {
        obj.values().cloned().collect()
    } else {
        vec![]
    };

    // Sort by info.time.created to ensure chronological order.
    all_messages.sort_by(|a, b| {
        let time_a = a
            .pointer("/info/time/created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let time_b = b
            .pointer("/info/time/created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        time_a.cmp(&time_b)
    });

    let total = all_messages.len();

    // Apply pagination: filter by `before` timestamp, then take last `limit`.
    if limit > 0 || before.is_some() {
        // Filter by `before` — keep only messages with created < before
        if let Some(before_ts) = before {
            all_messages.retain(|m| {
                let ts = m
                    .pointer("/info/time/created")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                ts < before_ts
            });
        }

        let filtered_count = all_messages.len();
        let effective_limit = if limit > 0 { limit } else { filtered_count };

        // Take only the last `limit` messages (most recent within the filtered set)
        let has_more = filtered_count > effective_limit;
        if has_more {
            all_messages = all_messages.split_off(filtered_count - effective_limit);
        }

        serde_json::json!({
            "messages": all_messages,
            "has_more": has_more,
            "total": total,
        })
    } else {
        // No pagination — return everything (backward compatible)
        serde_json::json!({
            "messages": all_messages,
            "has_more": false,
            "total": total,
        })
    }
}
