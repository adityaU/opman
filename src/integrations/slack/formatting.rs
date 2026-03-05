//! Markdown-to-Slack-mrkdwn conversion and Block Kit rendering helpers.

/// Render a list of `TodoItem`s as plain markdown checklist text.
///
/// Each item is rendered as a standard markdown checkbox:
/// - completed  → `- [x]`
/// - in_progress → `- [-]`
/// - cancelled  → `- [~]`
/// - pending    → `- [ ]`
///
/// High-priority items are suffixed with `[HIGH]`.
pub fn render_todos_mrkdwn(todos: &[crate::app::TodoItem]) -> String {
    if todos.is_empty() {
        return "*Todo List*\n_No items yet._".to_string();
    }

    let mut lines = Vec::with_capacity(todos.len() + 2);
    lines.push("*Todo List*".to_string());
    lines.push(String::new());

    for item in todos {
        let checkbox = match item.status.as_str() {
            "completed" => "- [x]",
            "in_progress" => "- [-]",
            "cancelled" => "- [~]",
            _ => "- [ ]", // pending / unknown
        };

        let priority_suffix = match item.priority.as_str() {
            "high" => "  `[HIGH]`",
            _ => "",
        };

        let content = if item.status == "completed" || item.status == "cancelled" {
            format!("~{}~", item.content) // strikethrough
        } else {
            item.content.clone()
        };

        lines.push(format!("{} {}{}", checkbox, content, priority_suffix));
    }

    // Add a summary line.
    let done = todos.iter().filter(|t| t.status == "completed").count();
    let total = todos.len();
    lines.push(format!("\n_{}/{} completed_", done, total));
    lines.join("\n")
}

/// Render a permission request as Slack mrkdwn text.
///
/// Shows the permission type, patterns, and instructions for how to reply.
#[allow(dead_code)]
pub fn render_permission_mrkdwn(req: &crate::app::PermissionRequest) -> String {
    let emoji = match req.permission.as_str() {
        "edit" => ":pencil2:",
        "bash" => ":terminal:",
        "read" => ":eyes:",
        "glob" | "grep" => ":mag:",
        "task" => ":robot_face:",
        "webfetch" | "websearch" => ":globe_with_meridians:",
        "external_directory" => ":file_folder:",
        "doom_loop" => ":warning:",
        _ => ":lock:",
    };

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

/// Render a question request as Slack mrkdwn text.
///
/// Shows each question with numbered options and instructions for replying.
#[allow(dead_code)]
pub fn render_question_mrkdwn(req: &crate::app::QuestionRequest) -> String {
    let mut lines = vec![":question: *Question from the AI agent*".to_string()];

    for (qi, q) in req.questions.iter().enumerate() {
        if req.questions.len() > 1 {
            lines.push(format!("\n*Question {}:*", qi + 1));
        }
        if !q.header.is_empty() {
            lines.push(format!("*{}*", q.header));
        }
        if !q.question.is_empty() {
            lines.push(q.question.clone());
        }

        if !q.options.is_empty() {
            lines.push(String::new());
            for (oi, opt) in q.options.iter().enumerate() {
                let desc = if opt.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", opt.description)
                };
                lines.push(format!("`{}`. {}{}", oi + 1, opt.label, desc));
            }
        }

        if q.multiple {
            lines.push(
                "\n_Multiple selections allowed — reply with comma-separated numbers (e.g. `1,3`)_"
                    .to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push(
        "Reply in this thread with the option number(s), or type a custom answer.".to_string(),
    );
    lines.push("Reply `reject` to dismiss this question.".to_string());

    lines.join("\n")
}

/// Render a permission request as Block Kit blocks with interactive buttons.
///
/// Returns `(fallback_text, blocks)` where `fallback_text` is used for
/// notifications and `blocks` is the Block Kit layout with action buttons.
pub fn render_permission_blocks(
    req: &crate::app::PermissionRequest,
) -> (String, Vec<serde_json::Value>) {
    let emoji = match req.permission.as_str() {
        "edit" => ":pencil2:",
        "bash" => ":terminal:",
        "read" => ":eyes:",
        "glob" | "grep" => ":mag:",
        "task" => ":robot_face:",
        "webfetch" | "websearch" => ":globe_with_meridians:",
        "external_directory" => ":file_folder:",
        "doom_loop" => ":warning:",
        _ => ":lock:",
    };

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

/// Render a question request as Block Kit blocks with interactive buttons.
///
/// Returns `(fallback_text, blocks)` where `blocks` contains section blocks
/// for the question text and actions blocks with option buttons.
///
/// Slack limits actions blocks to 25 elements each, so we split if needed.
pub fn render_question_blocks(
    req: &crate::app::QuestionRequest,
) -> (String, Vec<serde_json::Value>) {
    let fallback = "Question from the AI agent".to_string();
    let req_id = &req.id;
    let mut blocks: Vec<serde_json::Value> = Vec::new();

    // Header
    blocks.push(serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": ":question: *Question from the AI agent*"
        }
    }));

    for (qi, q) in req.questions.iter().enumerate() {
        // Question header/text
        let mut q_text_parts: Vec<String> = Vec::new();
        if req.questions.len() > 1 {
            q_text_parts.push(format!("*Question {}:*", qi + 1));
        }
        if !q.header.is_empty() {
            q_text_parts.push(format!("*{}*", q.header));
        }
        if !q.question.is_empty() {
            q_text_parts.push(q.question.clone());
        }
        if q.multiple {
            q_text_parts.push("_Multiple selections allowed_".to_string());
        }

        if !q_text_parts.is_empty() {
            blocks.push(serde_json::json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": q_text_parts.join("\n")
                }
            }));
        }

        // Option buttons — up to 25 per actions block (Slack limit is 25 elements).
        // We also add a description below each button if available.
        if !q.options.is_empty() {
            let mut buttons: Vec<serde_json::Value> = Vec::new();
            for (oi, opt) in q.options.iter().enumerate() {
                let label = if opt.label.len() > 75 {
                    // Slack button text limit is 75 chars
                    format!("{}…", &opt.label[..72])
                } else {
                    opt.label.clone()
                };
                let btn = serde_json::json!({
                    "type": "button",
                    "text": { "type": "plain_text", "text": label, "emoji": true },
                    "action_id": format!("q_{}_{}:{}", req_id, qi, oi),
                    "value": format!("{}:{}", qi, oi)
                });
                buttons.push(btn);
            }

            // Split into chunks of 24 (leaving room for a Dismiss button in last chunk)
            let chunks: Vec<&[serde_json::Value]> = buttons.chunks(24).collect();
            let num_chunks = chunks.len();
            for (ci, chunk) in chunks.iter().enumerate() {
                let mut elements: Vec<serde_json::Value> = chunk.to_vec();
                // Add dismiss button to the last chunk
                if ci == num_chunks - 1 {
                    elements.push(serde_json::json!({
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Dismiss", "emoji": true },
                        "style": "danger",
                        "action_id": format!("q_reject:{}", req_id),
                        "value": "reject"
                    }));
                }
                blocks.push(serde_json::json!({
                    "type": "actions",
                    "elements": elements
                }));
            }

            // If there are descriptions, show them as context
            let desc_parts: Vec<String> = q
                .options
                .iter()
                .enumerate()
                .filter(|(_, o)| !o.description.is_empty())
                .map(|(_oi, o)| format!("*{}*: {}", o.label, o.description))
                .collect();
            if !desc_parts.is_empty() {
                blocks.push(serde_json::json!({
                    "type": "context",
                    "elements": [{
                        "type": "mrkdwn",
                        "text": desc_parts.join("  |  ")
                    }]
                }));
            }
        }
    }

    // Fallback text for thread replies from users
    blocks.push(serde_json::json!({
        "type": "context",
        "elements": [{
            "type": "mrkdwn",
            "text": "_You can also reply in this thread with option numbers or custom text._"
        }]
    }));

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

    let perm_emoji = match req.permission.as_str() {
        "edit" => ":pencil2:",
        "bash" => ":terminal:",
        "read" => ":eyes:",
        "glob" | "grep" => ":mag:",
        "task" => ":robot_face:",
        "webfetch" | "websearch" => ":globe_with_meridians:",
        "external_directory" => ":file_folder:",
        "doom_loop" => ":warning:",
        _ => ":lock:",
    };

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

/// Build a "confirmed" version of a question message (buttons removed).
pub fn render_question_confirmed_blocks(answer_display: &str) -> (String, Vec<serde_json::Value>) {
    let fallback = format!("Question answered: {}", answer_display);
    let text = format!(":white_check_mark: *Answer sent:* {}", answer_display);

    let blocks = vec![serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": text
        }
    })];

    (fallback, blocks)
}

/// Build a "dismissed" version of a question message (buttons removed).
pub fn render_question_dismissed_blocks() -> (String, Vec<serde_json::Value>) {
    let fallback = "Question dismissed".to_string();
    let text = ":no_entry_sign: *Question dismissed*".to_string();

    let blocks = vec![serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": text
        }
    })];

    (fallback, blocks)
}

// ── Markdown → Slack mrkdwn ────────────────────────────────────────────

/// Convert standard Markdown to Slack mrkdwn format.
///
/// Key differences handled:
/// - `**bold**` → `*bold*`
/// - `*italic*` (when not inside bold) → `_italic_`
/// - `~~strike~~` → `~strike~`
/// - `[text](url)` → `<url|text>`
/// - `# heading` → `*heading*` (bold, Slack has no headings)
/// - Markdown tables → fenced code blocks (Slack has no table syntax)
/// - Fenced code blocks and inline code pass through unchanged.
pub fn markdown_to_slack_mrkdwn(md: &str) -> String {
    // Pre-pass: convert markdown tables to code blocks.
    let preprocessed = convert_markdown_tables(md);
    convert_inline_markdown(&preprocessed)
}

/// Detect markdown tables and wrap them in fenced code blocks so they render
/// as monospace in Slack (which has no native table support).
///
/// A markdown table is identified as a sequence of lines where:
/// - Each line contains at least one `|` character
/// - One of the early lines is a separator row (e.g. `|---|---|`)
pub fn convert_markdown_tables(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut result = String::with_capacity(md.len() + 128);
    let mut i = 0;
    let total = lines.len();
    // Track whether the original text ended with a newline.
    let ends_with_newline = md.ends_with('\n');

    while i < total {
        let line = lines[i];

        // Check if this line looks like a table row (contains `|`).
        // Also require the next line to be a separator row for a valid table.
        if line.contains('|') && i + 1 < total && is_table_separator(lines[i + 1]) {
            // Found a table. Collect all contiguous table rows.
            let table_start = i;
            let mut table_end = i;
            while table_end < total && is_table_line(lines[table_end]) {
                table_end += 1;
            }

            // Check if already inside a code fence (crude check: look back for
            // unmatched ```). We skip conversion if so.
            let preceding = &result;
            let fence_count = preceding.matches("```").count();
            if fence_count % 2 == 1 {
                // Inside a code block — pass through as-is.
                for j in table_start..table_end {
                    result.push_str(lines[j]);
                    result.push('\n');
                }
            } else {
                result.push_str("```\n");
                for j in table_start..table_end {
                    result.push_str(lines[j]);
                    result.push('\n');
                }
                result.push_str("```\n");
            }
            i = table_end;
        } else {
            result.push_str(line);
            if i + 1 < total || ends_with_newline {
                result.push('\n');
            }
            i += 1;
        }
    }

    result
}

/// Check if a line is a markdown table separator row (e.g. `| --- | --- |` or `|:---:|---:|`).
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }
    // After removing `|`, `:`, `-`, and whitespace, nothing should remain.
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '|' | '-' | ':' | ' '))
        .collect();
    cleaned.is_empty() && trimmed.contains('-')
}

/// Check if a line looks like part of a markdown table (contains `|`).
fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    // A table line should contain `|`. Empty lines break the table.
    !trimmed.is_empty() && trimmed.contains('|')
}

/// The character-level inline markdown to Slack mrkdwn converter.
fn convert_inline_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut chars = md.chars().peekable();
    // Track whether we are inside a fenced code block (```).
    let mut in_fenced_block = false;
    // Track whether we are inside an inline code span (`).
    let mut in_inline_code = false;
    // Track whether we are at the start of a line (for heading detection).
    let mut at_line_start = true;

    while let Some(c) = chars.next() {
        // ── Fenced code block toggle ───────────────────────────────
        if c == '`' && chars.peek() == Some(&'`') {
            // Check for triple backtick.
            let second = chars.next(); // consume 2nd `
            if chars.peek() == Some(&'`') {
                let third = chars.next(); // consume 3rd `
                in_fenced_block = !in_fenced_block;
                result.push(c);
                if let Some(ch) = second {
                    result.push(ch);
                }
                if let Some(ch) = third {
                    result.push(ch);
                }
                at_line_start = false;
                continue;
            } else {
                // Only two backticks — not a fence, push them through.
                result.push(c);
                if let Some(ch) = second {
                    result.push(ch);
                }
                at_line_start = false;
                continue;
            }
        }

        // Inside fenced code blocks, pass everything through verbatim.
        if in_fenced_block {
            if c == '\n' {
                at_line_start = true;
            } else {
                at_line_start = false;
            }
            result.push(c);
            continue;
        }

        // ── Inline code toggle ─────────────────────────────────────
        if c == '`' {
            in_inline_code = !in_inline_code;
            result.push(c);
            at_line_start = false;
            continue;
        }

        // Inside inline code, pass through verbatim.
        if in_inline_code {
            result.push(c);
            at_line_start = false;
            continue;
        }

        // ── Headings at line start ─────────────────────────────────
        if c == '#' && at_line_start {
            // Consume all leading '#' and optional space.
            while chars.peek() == Some(&'#') {
                chars.next();
            }
            if chars.peek() == Some(&' ') {
                chars.next();
            }
            // Collect the heading text until end of line.
            let mut heading = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '\n' {
                    break;
                }
                heading.push(chars.next().unwrap());
            }
            // Render as bold in Slack.
            result.push('*');
            result.push_str(heading.trim());
            result.push('*');
            at_line_start = false;
            continue;
        }

        // ── Bold: **text** → *text* ────────────────────────────────
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            result.push('*');
            at_line_start = false;
            continue;
        }

        // ── Italic: standalone *text* → _text_ ────────────────────
        // At this point a single `*` that wasn't part of `**` is italic.
        if c == '*' {
            result.push('_');
            at_line_start = false;
            continue;
        }

        // ── Strikethrough: ~~text~~ → ~text~ ───────────────────────
        if c == '~' && chars.peek() == Some(&'~') {
            chars.next(); // consume second ~
            result.push('~');
            at_line_start = false;
            continue;
        }

        // ── Links: [text](url) → <url|text> ───────────────────────
        if c == '[' {
            // Try to parse a markdown link.
            let mut link_text = String::new();
            let mut found_link = false;
            let mut inner_chars = chars.clone();
            let mut bracket_depth = 1;

            // Collect text inside brackets.
            while let Some(ch) = inner_chars.next() {
                if ch == '[' {
                    bracket_depth += 1;
                } else if ch == ']' {
                    bracket_depth -= 1;
                    if bracket_depth == 0 {
                        // Check if followed by (url)
                        if inner_chars.peek() == Some(&'(') {
                            inner_chars.next(); // consume '('
                            let mut url = String::new();
                            let mut paren_depth = 1;
                            while let Some(uc) = inner_chars.next() {
                                if uc == '(' {
                                    paren_depth += 1;
                                    url.push(uc);
                                } else if uc == ')' {
                                    paren_depth -= 1;
                                    if paren_depth == 0 {
                                        found_link = true;
                                        break;
                                    }
                                    url.push(uc);
                                } else {
                                    url.push(uc);
                                }
                            }
                            if found_link {
                                result.push('<');
                                result.push_str(&url);
                                result.push('|');
                                result.push_str(&link_text);
                                result.push('>');
                                chars = inner_chars;
                            }
                        }
                        break;
                    }
                }
                if bracket_depth > 0 {
                    link_text.push(ch);
                }
            }
            if !found_link {
                result.push(c);
            }
            at_line_start = false;
            continue;
        }

        // ── Newline tracking ───────────────────────────────────────
        if c == '\n' {
            at_line_start = true;
            result.push(c);
            continue;
        }

        at_line_start = false;
        result.push(c);
    }

    result
}
