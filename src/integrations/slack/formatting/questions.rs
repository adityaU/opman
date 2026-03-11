//! Question-request rendering helpers (mrkdwn text and Block Kit).

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
