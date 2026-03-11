//! Permission-request rendering helpers (mrkdwn text and Block Kit).

/// Map a permission type to its Slack emoji.
fn permission_emoji(permission: &str) -> &'static str {
    match permission {
        "edit" => ":pencil2:",
        "bash" => ":terminal:",
        "read" => ":eyes:",
        "glob" | "grep" => ":mag:",
        "task" => ":robot_face:",
        "webfetch" | "websearch" => ":globe_with_meridians:",
        "external_directory" => ":file_folder:",
        "doom_loop" => ":warning:",
        _ => ":lock:",
    }
}

/// Render a permission request as Slack mrkdwn text.
///
/// Shows the permission type, patterns, and instructions for how to reply.
#[allow(dead_code)]
pub fn render_permission_mrkdwn(req: &crate::app::PermissionRequest) -> String {
    let emoji = permission_emoji(&req.permission);

    let mut lines = vec![format!(
        "{} *Permission requested:* `{}`",
        emoji, req.permission
    )];

    if !req.patterns.is_empty() {
        let patterns_str = req
            .patterns
            .iter()
            .map(|p| format!("`{}`", p))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("Patterns: {}", patterns_str));
    }

    // Show metadata if present and meaningful.
    if let Some(obj) = req.metadata.as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    lines.push(format!("{}: `{}`", k, s));
                }
            }
        }
    }

    lines.push(String::new());
    lines.push("Reply in this thread:".to_string());
    lines.push("`once` — allow this one time".to_string());
    lines.push("`always` — allow for the rest of the session".to_string());
    lines.push("`reject` — deny this request".to_string());
    lines.join("\n")
}

/// Render a permission request as Block Kit blocks with interactive buttons.
///
/// Returns `(fallback_text, blocks)` where `fallback_text` is used for
/// notifications and `blocks` is the Block Kit layout with action buttons.
pub fn render_permission_blocks(
    req: &crate::app::PermissionRequest,
) -> (String, Vec<serde_json::Value>) {
    let emoji = permission_emoji(&req.permission);

    let mut text_lines = vec![format!(
        "{} *Permission requested:* `{}`",
        emoji, req.permission
    )];

    if !req.patterns.is_empty() {
        let patterns_str = req
            .patterns
            .iter()
            .map(|p| format!("`{}`", p))
            .collect::<Vec<_>>()
            .join(", ");
        text_lines.push(format!("Patterns: {}", patterns_str));
    }

    if let Some(obj) = req.metadata.as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    text_lines.push(format!("{}: `{}`", k, s));
                }
            }
        }
    }

    let fallback = format!("Permission requested: {}", req.permission);
    let detail_text = text_lines.join("\n");
    let req_id = &req.id;

    let blocks = vec![
        serde_json::json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": detail_text
            }
        }),
        serde_json::json!({
            "type": "actions",
            "elements": [
                {
                    "type": "button",
                    "text": { "type": "plain_text", "text": "Once", "emoji": true },
                    "style": "primary",
                    "action_id": format!("perm_once:{}", req_id),
                    "value": "once"
                },
                {
                    "type": "button",
                    "text": { "type": "plain_text", "text": "Always", "emoji": true },
                    "action_id": format!("perm_always:{}", req_id),
                    "value": "always"
                },
                {
                    "type": "button",
                    "text": { "type": "plain_text", "text": "Reject", "emoji": true },
                    "style": "danger",
                    "action_id": format!("perm_reject:{}", req_id),
                    "value": "reject"
                }
            ]
        }),
    ];

    (fallback, blocks)
}

/// Build a "confirmed" version of a permission message (buttons removed).
pub fn render_permission_confirmed_blocks(
    req: &crate::app::PermissionRequest,
    action: &str,
) -> (String, Vec<serde_json::Value>) {
    let emoji = match action {
        "once" | "always" => ":white_check_mark:",
        "reject" => ":no_entry_sign:",
        _ => ":white_check_mark:",
    };

    let perm_emoji = permission_emoji(&req.permission);

    let fallback = format!("Permission {}: {}", action, req.permission);
    let text = format!(
        "{} *Permission:* `{}` — {} *{}*",
        perm_emoji, req.permission, emoji, action
    );

    let blocks = vec![serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": text
        }
    })];

    (fallback, blocks)
}
