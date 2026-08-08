//! The `ask_user_question` tool: schema, the loopback round-trip, and the answer text.

use serde_json::{json, Value};

use crate::loopback::Loopback;

/// Ceiling on how many questions one card may carry. More than a handful stops being a
/// decision point and becomes a form, which is a worse experience than asking twice.
const MAX_QUESTIONS: usize = 4;

pub(super) const TOOL_NAME: &str = "ask_user_question";

/// What the user is told when opman is unreachable — actionable, because the agent can
/// still make progress by choosing a default rather than retrying a dead endpoint.
const UNAVAILABLE: &str = "Cannot ask the user: the opman web server is not running (no \
     ~/.config/opman/internal.json). Choose the most reasonable default, say which default \
     you chose and why, and continue.";

const DISMISSED: &str = "The user dismissed the question without answering. Choose the \
     most reasonable default, say which default you chose and why, and continue. Do not \
     ask this again.";

pub(super) fn definitions() -> Value {
    let option = json!({
        "type": "object",
        "properties": {
            "label": { "type": "string", "description": "Short button text for this choice." },
            "description": { "type": "string", "description": "One line on what picking this means." },
        },
        "required": ["label"],
    });
    json!([{
        "name": TOOL_NAME,
        "description": include_str!("tool_description.txt"),
        "inputSchema": {
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_QUESTIONS,
                    "description": "The questions to ask, decided together on one card.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": { "type": "string", "description": "The question in full, as a self-contained sentence." },
                            "header": { "type": "string", "description": "Tab label, 12 characters or fewer." },
                            "options": { "type": "array", "minItems": 2, "maxItems": 4, "items": option },
                            "multiSelect": { "type": "boolean", "description": "Allow more than one option to be chosen." },
                        },
                        "required": ["question", "header", "options"],
                    },
                },
            },
            "required": ["questions"],
        },
    }])
}

/// The questions a `tools/call` carries, rejected here rather than at the web server so a
/// malformed call costs no round-trip and the agent gets a schema-shaped complaint.
pub(super) fn questions(params: Option<&Value>) -> Result<Vec<Value>, String> {
    let raw = params
        .and_then(|p| p.pointer("/arguments/questions"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if raw.is_empty() {
        return Err("`questions` must be a non-empty array of question objects.".to_string());
    }
    if raw.len() > MAX_QUESTIONS {
        return Err(format!(
            "Ask at most {MAX_QUESTIONS} questions in one call; split the rest into a later call."
        ));
    }
    if let Some(bad) = raw.iter().position(|q| !is_answerable(q)) {
        return Err(format!(
            "Question {} needs a `question` string and at least two `options`, each with a `label`.",
            bad + 1
        ));
    }
    Ok(raw.to_vec())
}

/// A question with no text, or with fewer than two labelled options, is not a choice —
/// the card would render as a prompt the user cannot act on.
fn is_answerable(question: &Value) -> bool {
    let has_text = question
        .get("question")
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty());
    let labelled = question
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter(|o| {
                    o.get("label")
                        .and_then(Value::as_str)
                        .is_some_and(|l| !l.trim().is_empty())
                })
                .count()
        })
        .unwrap_or(0);
    has_text && labelled >= 2
}

/// Raise the card and wait. The HTTP call is the wait: opman answers it only once the user
/// has replied, been dismissed, or the request has aged out.
pub(super) async fn ask(
    loopback: Option<&Loopback>,
    session: Option<&str>,
    directory: &str,
    questions: Vec<Value>,
) -> String {
    let Some(loopback) = loopback else {
        return UNAVAILABLE.to_string();
    };
    let body = json!({
        "sessionID": session.unwrap_or_default(),
        "directory": directory,
        "questions": &questions,
    });
    let response = loopback.post("/internal/ask").json(&body).send().await;
    let payload = match response {
        Ok(response) if response.status().is_success() => response.json::<Value>().await.ok(),
        Ok(response) => {
            return format!(
                "Could not ask the user (opman returned {}). Choose a reasonable default and continue.",
                response.status()
            )
        }
        Err(error) => {
            return format!(
                "Could not reach opman to ask the user ({error}). Choose a reasonable default and continue."
            )
        }
    };
    let answers = payload
        .as_ref()
        .and_then(|p| p.get("answers"))
        .and_then(Value::as_array);
    match answers {
        Some(answers) if answers.iter().any(has_selection) => format_answers(&questions, answers),
        _ => DISMISSED.to_string(),
    }
}

fn has_selection(answer: &Value) -> bool {
    answer.as_array().is_some_and(|picked| {
        picked
            .iter()
            .any(|v| v.as_str().is_some_and(|s| !s.is_empty()))
    })
}

/// Pair each answer back with the question it belongs to. Positional alone would make the
/// agent re-derive which answer went with which question from its own call.
fn format_answers(questions: &[Value], answers: &[Value]) -> String {
    let mut out = String::from("The user answered:\n");
    for (index, question) in questions.iter().enumerate() {
        let text = question
            .get("question")
            .and_then(Value::as_str)
            .unwrap_or("(question)");
        let picked: Vec<&str> = answers
            .get(index)
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let picked = match picked.is_empty() {
            true => "(no answer)".to_string(),
            false => picked.join(", "),
        };
        out.push_str(&format!("- {text} → {picked}\n"));
    }
    out.push_str("Treat these as decided and continue. Do not ask the same question again.");
    out
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tools_tests;
