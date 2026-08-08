//! ACP content blocks → what the timeline renders.
//!
//! opman used to read one field off an agent's content — `content.text` — and drop the rest,
//! which is why an agent that answered with an image produced an empty message. The protocol
//! allows five kinds of block anywhere content appears, and opman already *sends* all five
//! (see [`super::attach`]); this is the same vocabulary read in the other direction.

use serde_json::Value;

/// What one block becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendered {
    /// Prose, folded into the streaming text part like any other chunk.
    Text(String),
    /// A payload that is not prose. Becomes a `file` part, which the timeline already renders
    /// as a preview for the attachments a user sends.
    File {
        mime: String,
        filename: String,
        url: String,
    },
}

/// Read one content block. `None` for a block with nothing to show — an empty string, or a
/// type from a newer revision, neither of which is a reason to break the message.
pub fn render(block: &Value) -> Option<Rendered> {
    let text = |value: &str| (!value.is_empty()).then(|| Rendered::Text(value.to_string()));
    match block.get("type").and_then(Value::as_str)? {
        "text" => text(block.get("text").and_then(Value::as_str)?),
        "image" | "audio" => payload(block, block.get("data")?.as_str()?),
        // A link is a name and a place, not content: rendering it as a link is the whole of
        // what the agent said, and keeps the path clickable in the timeline.
        "resource_link" => {
            let uri = block.get("uri").and_then(Value::as_str)?;
            let name = block
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or(uri);
            text(&format!("[{name}]({uri})"))
        }
        "resource" => embedded(block.get("resource")?),
        _ => None,
    }
}

/// An embedded resource: its text when it has any, else its bytes.
fn embedded(resource: &Value) -> Option<Rendered> {
    if let Some(text) = resource.get("text").and_then(Value::as_str) {
        return (!text.is_empty()).then(|| Rendered::Text(text.to_string()));
    }
    payload(resource, resource.get("blob")?.as_str()?)
}

/// A base64 payload as a data URL, named after the URI it came with when it has one.
fn payload(block: &Value, data: &str) -> Option<Rendered> {
    if data.is_empty() {
        return None;
    }
    let mime = block
        .get("mimeType")
        .and_then(Value::as_str)
        .filter(|mime| !mime.is_empty())
        .unwrap_or("application/octet-stream");
    let uri = block.get("uri").and_then(Value::as_str).unwrap_or_default();
    Some(Rendered::File {
        filename: filename_of(uri, mime),
        url: format!("data:{mime};base64,{data}"),
        mime: mime.to_string(),
    })
}

/// A name to show. The URI's last segment when there is one, else the media type — an inline
/// image arrives with no name at all, and "image/png" says more than an empty label.
fn filename_of(uri: &str, mime: &str) -> String {
    uri.rsplit('/')
        .find(|segment| !segment.is_empty())
        .filter(|_| !uri.is_empty())
        .unwrap_or(mime)
        .to_string()
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod content_tests;
