//! Attachments on a user prompt: the composer's parts in, ACP content blocks out.
//!
//! The composer posts an image or file as an opencode `file` part carrying a data URL. ACP
//! has its own vocabulary for the same thing — `image`, `resource`, `resource_link` — and an
//! agent declares which of those it accepts in `promptCapabilities`. This module is the
//! translation, and it is deliberately the only place that knows either shape.
//!
//! Nothing is silently dropped. An attachment an agent cannot receive is still named in the
//! prompt as a `resource_link`, which every ACP agent must accept: the model then knows a
//! file was attached and can ask for it, rather than answering a question about an image it
//! was never told existed.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};

/// What the agent said it can receive in a prompt, from `initialize`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PromptCaps {
    pub image: bool,
    pub audio: bool,
    /// `ContentBlock::Resource` — a file's contents inlined into the prompt.
    pub embedded_context: bool,
}

impl PromptCaps {
    /// Read the capabilities out of an `initialize` reply. Absent means unsupported, which
    /// is the spec's default and the safe reading for an agent that predates a field.
    pub fn from_initialize(init: &Value) -> Self {
        let caps = init
            .get("agentCapabilities")
            .and_then(|c| c.get("promptCapabilities"));
        let flag = |key: &str| {
            caps.and_then(|c| c.get(key))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        Self {
            image: flag("image"),
            audio: flag("audio"),
            embedded_context: flag("embeddedContext"),
        }
    }
}

/// One file the user attached to a prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub filename: String,
    pub mime: String,
    /// Base64 payload, exactly as it arrived in the data URL.
    pub data: String,
}

/// A user turn: what to say, and what came with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prompt {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

impl Prompt {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            attachments: Vec::new(),
        }
    }

    /// Nothing to send: no prose and no files.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.attachments.is_empty()
    }

    /// Pull a prompt out of a send body: `parts[]` of type `text` and `file`, falling back
    /// to a bare `text`/`prompt` string.
    pub fn from_body(body: &Value) -> Self {
        let Some(parts) = body.get("parts").and_then(Value::as_array) else {
            return Self::text(fallback_text(body));
        };
        let mut text = Vec::new();
        let mut attachments = Vec::new();
        for part in parts {
            match part.get("type").and_then(Value::as_str).unwrap_or("text") {
                "text" => {
                    if let Some(value) = part.get("text").and_then(Value::as_str) {
                        text.push(value);
                    }
                }
                "file" => {
                    if let Some(attachment) = Attachment::from_part(part) {
                        attachments.push(attachment);
                    }
                }
                _ => {}
            }
        }
        let text = text.join("\n");
        if text.is_empty() && attachments.is_empty() {
            return Self::text(fallback_text(body));
        }
        Self { text, attachments }
    }

    /// The `prompt` array for `session/prompt`.
    ///
    /// The text block is always present, even when empty: an agent handed only an image
    /// still needs a turn to respond to, and some reject a prompt with no text at all.
    pub fn content_blocks(&self, caps: PromptCaps) -> Vec<Value> {
        let mut blocks = vec![json!({ "type": "text", "text": self.text })];
        blocks.extend(self.attachments.iter().map(|a| a.content_block(caps)));
        blocks
    }

    /// The opencode parts for the rendered user message, so the transcript shows the same
    /// attachments the agent received.
    pub fn message_parts(&self, message_id: &str, session_id: &str) -> Vec<Value> {
        let mut parts = vec![json!({
            "type": "text",
            "id": format!("{message_id}:0"),
            "messageID": message_id,
            "sessionID": session_id,
            "text": self.text,
        })];
        parts.extend(self.attachments.iter().enumerate().map(|(i, a)| {
            a.message_part(message_id, session_id, i + 1)
        }));
        parts
    }
}

impl Attachment {
    /// Read an opencode `file` part: `{ type, mime, url: "data:<mime>;base64,<data>", filename }`.
    fn from_part(part: &Value) -> Option<Self> {
        let url = part.get("url").and_then(Value::as_str)?;
        let (declared_mime, data) = split_data_url(url)?;
        let mime = part
            .get("mime")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
            .unwrap_or(declared_mime)
            .to_string();
        let filename = part
            .get("filename")
            .and_then(Value::as_str)
            .filter(|f| !f.is_empty())
            .unwrap_or("attachment")
            .to_string();
        Some(Self {
            filename,
            mime,
            data: data.to_string(),
        })
    }

    pub fn is_image(&self) -> bool {
        self.mime.starts_with("image/")
    }

    /// A `file://` URI for the attachment. ACP resource blocks are keyed by URI, and the
    /// filename is all opman knows: the browser never sends a path.
    fn uri(&self) -> String {
        format!("file:///{}", self.filename.trim_start_matches('/'))
    }

    /// The ACP block for this attachment, chosen against what the agent accepts.
    fn content_block(&self, caps: PromptCaps) -> Value {
        if self.is_image() && caps.image {
            return json!({ "type": "image", "mimeType": self.mime, "data": self.data });
        }
        if self.mime.starts_with("audio/") && caps.audio {
            return json!({ "type": "audio", "mimeType": self.mime, "data": self.data });
        }
        // Text-ish content can be inlined, which is what makes an attached log or diff
        // actually readable to the agent rather than a name it has to go and fetch.
        if caps.embedded_context {
            if let Some(text) = self.as_text() {
                return json!({
                    "type": "resource",
                    "resource": { "uri": self.uri(), "mimeType": self.mime, "text": text },
                });
            }
            return json!({
                "type": "resource",
                "resource": { "uri": self.uri(), "mimeType": self.mime, "blob": self.data },
            });
        }
        // The baseline every ACP agent must accept.
        json!({
            "type": "resource_link",
            "uri": self.uri(),
            "name": self.filename,
            "mimeType": self.mime,
        })
    }

    /// Decoded UTF-8 contents, when the payload is text at all.
    fn as_text(&self) -> Option<String> {
        if self.is_image() || self.mime.starts_with("audio/") || self.mime.starts_with("video/") {
            return None;
        }
        String::from_utf8(BASE64.decode(&self.data).ok()?).ok()
    }

    /// The opencode part the message timeline renders as a preview.
    fn message_part(&self, message_id: &str, session_id: &str, index: usize) -> Value {
        json!({
            "type": "file",
            "id": format!("{message_id}:{index}"),
            "messageID": message_id,
            "sessionID": session_id,
            "mime": self.mime,
            "filename": self.filename,
            "url": format!("data:{};base64,{}", self.mime, self.data),
        })
    }
}

/// Split `data:<mime>[;base64],<payload>` into its mime type and payload. Only base64
/// payloads are accepted — that is what the composer sends, and guessing at a percent-encoded
/// alternative would risk handing the agent corrupted bytes.
fn split_data_url(url: &str) -> Option<(&str, &str)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    Some((mime, data))
}

fn fallback_text(body: &Value) -> String {
    body.get("text")
        .or_else(|| body.get("prompt"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod attach_tests;
