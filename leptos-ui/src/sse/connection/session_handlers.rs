//! Session, permission, question, and file-edit event handlers.
//! Called from `event_handler::handle_opencode_event`.

use leptos::prelude::{GetUntracked, Set, Update};

use crate::hooks::use_sse_state::{SessionStatus, SseState};

pub fn handle_session_status(sse: &SseState, props: &serde_json::Value) {
    let sid = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let status_str = props
        .get("status")
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if let Some(obj) = v.as_object() {
                obj.get("type")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    let is_busy = status_str == "busy" || status_str == "retry";

    if !sid.is_empty() {
        let sid_owned = sid.to_string();
        sse.set_busy_sessions
            .update(move |set: &mut std::collections::HashSet<String>| {
                if is_busy {
                    set.insert(sid_owned);
                } else {
                    set.remove(&sid_owned);
                }
            });
    }

    let active_sid = sse.tracked_session_id();
    if active_sid.as_deref() == Some(sid) && !sid.is_empty() {
        let new_status = if is_busy {
            SessionStatus::Busy
        } else {
            SessionStatus::Idle
        };
        // Dedup: only set if actually different
        if sse.session_status.get_untracked() != new_status {
            sse.set_session_status.set(new_status);
        }
    }
}

pub fn handle_session_updated(sse: &SseState, props: &serde_json::Value) {
    let info = match props.get("info") {
        Some(i) => i,
        None => return,
    };
    let sid = info.get("id").and_then(|v| v.as_str()).unwrap_or_default();
    if sid.is_empty() {
        return;
    }
    let title = info
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let time_created = info
        .get("time")
        .and_then(|t| t.get("created"))
        .and_then(|v| v.as_f64());
    let time_updated = info
        .get("time")
        .and_then(|t| t.get("updated"))
        .and_then(|v| v.as_f64());
    let sid_owned = sid.to_string();

    // Only mutate app_state if something actually changed, to avoid
    // broadcasting reactivity to every subscriber for no-op updates.
    sse.set_app_state
        .update(move |state: &mut Option<crate::types::api::AppState>| {
            if let Some(ref mut s) = state {
                for project in &mut s.projects {
                    for session in &mut project.sessions {
                        if session.id == sid_owned {
                            let mut changed = false;
                            if let Some(ref t) = title {
                                if session.title != *t {
                                    session.title = t.clone();
                                    changed = true;
                                }
                            }
                            if let Some(created) = time_created {
                                if session.time.created != created {
                                    session.time.created = created;
                                    changed = true;
                                }
                            }
                            if let Some(updated) = time_updated {
                                if session.time.updated != updated {
                                    session.time.updated = updated;
                                    changed = true;
                                }
                            }
                            // If nothing changed, we still entered update() which
                            // will notify subscribers. To suppress notification on
                            // no-op we'd need try_update or skip pattern — but Leptos
                            // signal::update always marks dirty. The check still
                            // reduces derived memo invalidation since the Memo's
                            // PartialEq will see the same value.
                            let _ = changed;
                            return;
                        }
                    }
                }
            }
        });
}

/// Parse raw event properties into a `PermissionRequest`.
/// Reused by both the live SSE handler and the reload/fetch_pending path.
pub fn parse_permission_from_props(
    props: &serde_json::Value,
) -> Option<crate::types::core::PermissionRequest> {
    let id = props
        .get("id")
        .or_else(|| props.get("requestID"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if id.is_empty() {
        return None;
    }
    let session_id = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let tool_name = props
        .get("permission")
        .or_else(|| props.get("toolName"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let patterns: Option<Vec<String>> =
        props.get("patterns").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

    let metadata: Option<std::collections::HashMap<String, serde_json::Value>> = props
        .get("metadata")
        .and_then(|v| v.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());

    let description = format_permission_description(props, &tool_name, &patterns);

    Some(crate::types::core::PermissionRequest {
        id,
        session_id,
        tool_name,
        description,
        patterns,
        metadata,
        time: js_sys::Date::now(),
    })
}

pub fn handle_permission_asked(sse: &SseState, props: &serde_json::Value) {
    let Some(perm) = parse_permission_from_props(props) else {
        return;
    };
    let id = perm.id.clone();
    sse.set_permissions.update(
        move |perms: &mut Vec<crate::types::core::PermissionRequest>| {
            perms.retain(|p| p.id != id);
            perms.push(perm);
        },
    );
}

pub fn handle_permission_replied(sse: &SseState, props: &serde_json::Value) {
    let request_id = props
        .get("requestID")
        .or_else(|| props.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if !request_id.is_empty() {
        sse.set_permissions.update(
            move |perms: &mut Vec<crate::types::core::PermissionRequest>| {
                perms.retain(|p| p.id != request_id);
            },
        );
    }
}

/// Parse raw event properties into a `QuestionRequest`.
/// Reused by both the live SSE handler and the reload/fetch_pending path.
pub fn parse_question_from_props(
    props: &serde_json::Value,
) -> Option<crate::types::core::QuestionRequest> {
    let id = props
        .get("id")
        .or_else(|| props.get("requestID"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if id.is_empty() {
        return None;
    }
    let session_id = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let raw_questions = props
        .get("questions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let questions: Vec<crate::types::core::QuestionItem> =
        raw_questions.iter().map(transform_question_info).collect();

    let title = derive_question_title(props, &raw_questions);

    Some(crate::types::core::QuestionRequest {
        id,
        session_id,
        title,
        questions,
        time: js_sys::Date::now(),
    })
}

pub fn handle_question_asked(sse: &SseState, props: &serde_json::Value) {
    let Some(q) = parse_question_from_props(props) else {
        return;
    };
    let id = q.id.clone();
    sse.set_questions
        .update(move |qs: &mut Vec<crate::types::core::QuestionRequest>| {
            qs.retain(|q| q.id != id);
            qs.push(q);
        });
}

pub fn handle_question_replied(sse: &SseState, props: &serde_json::Value) {
    let request_id = props
        .get("requestID")
        .or_else(|| props.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if !request_id.is_empty() {
        sse.set_questions
            .update(move |qs: &mut Vec<crate::types::core::QuestionRequest>| {
                qs.retain(|q| q.id != request_id);
            });
    }
}

// ── Helper functions ────────────────────────────────────────────────

/// Derive a human-readable description for a permission request.
/// Uses explicit `description` field, else builds from permission name + patterns.
fn format_permission_description(
    props: &serde_json::Value,
    tool_name: &str,
    patterns: &Option<Vec<String>>,
) -> Option<String> {
    // Use explicit description if present
    if let Some(desc) = props.get("description").and_then(|v| v.as_str()) {
        if !desc.is_empty() {
            return Some(desc.to_string());
        }
    }
    let mut parts = Vec::new();
    if !tool_name.is_empty() {
        parts.push(tool_name.to_string());
    }
    if let Some(pats) = patterns {
        if !pats.is_empty() {
            parts.push(pats.join(", "));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(": "))
    }
}

/// Derive a title for a question request.
/// Priority: explicit title > first question header > "Question".
fn derive_question_title(props: &serde_json::Value, raw_questions: &[serde_json::Value]) -> String {
    if let Some(title) = props.get("title").and_then(|v| v.as_str()) {
        if !title.is_empty() {
            return title.to_string();
        }
    }
    if let Some(first) = raw_questions.first() {
        if let Some(header) = first.get("header").and_then(|v| v.as_str()) {
            if !header.is_empty() {
                return header.to_string();
            }
        }
    }
    "Question".to_string()
}

/// Transform an upstream question info JSON object into a `QuestionItem`.
///
/// Upstream sends: `{ question, header, options: [{label, description}], multiple, custom }`
/// Frontend expects: `{ text, header, type, options: [label], optionDescriptions: [desc], multiple, custom }`
fn transform_question_info(raw: &serde_json::Value) -> crate::types::core::QuestionItem {
    let question = raw
        .get("question")
        .or_else(|| raw.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let header = raw
        .get("header")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let multiple = raw
        .get("multiple")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // `custom` defaults to true (upstream opencode allows free-text alongside options)
    let custom = raw
        .get("custom")
        .map(|v| v.as_bool().unwrap_or(true))
        .unwrap_or(true);

    // Parse options: upstream sends [{label, description}] or plain strings
    let raw_options = raw.get("options").and_then(|v| v.as_array());
    let mut labels = Vec::new();
    let mut descs = Vec::new();
    if let Some(opts) = raw_options {
        for opt in opts {
            if let Some(s) = opt.as_str() {
                labels.push(s.to_string());
                descs.push(String::new());
            } else if let Some(obj) = opt.as_object() {
                labels.push(
                    obj.get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                );
                descs.push(
                    obj.get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                );
            }
        }
    }

    // Derive question type: explicit "confirm", or infer from options
    let question_type = if raw.get("type").and_then(|v| v.as_str()) == Some("confirm") {
        "confirm".to_string()
    } else if !labels.is_empty() {
        "select".to_string()
    } else {
        "text".to_string()
    };

    let options = if labels.is_empty() {
        None
    } else {
        Some(labels)
    };
    let option_descriptions = if descs.iter().any(|d| !d.is_empty()) {
        Some(descs)
    } else {
        None
    };

    crate::types::core::QuestionItem {
        text: question,
        header,
        question_type,
        options,
        option_descriptions,
        multiple: Some(multiple),
        custom: Some(custom),
    }
}

pub fn handle_file_edited(sse: &SseState, props: &serde_json::Value) {
    let edit_session_id = props
        .get("sessionID")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let active = sse.tracked_session_id();
    if edit_session_id.is_empty() || active.as_deref() == Some(edit_session_id) {
        sse.set_file_edit_count.update(|c: &mut usize| *c += 1);
    }
}
