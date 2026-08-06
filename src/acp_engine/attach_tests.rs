//! Attachment translation: composer parts in, ACP blocks and transcript parts out.

use super::*;

/// A one-pixel PNG, base64 — enough to prove the payload travels untouched.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUg==";

fn file_part(mime: &str, name: &str, data: &str) -> Value {
    json!({
        "type": "file",
        "mime": mime,
        "filename": name,
        "url": format!("data:{mime};base64,{data}"),
    })
}

fn body_with(parts: Vec<Value>) -> Value {
    json!({ "parts": parts })
}

const ALL: PromptCaps = PromptCaps {
    image: true,
    audio: true,
    embedded_context: true,
};
const NONE: PromptCaps = PromptCaps {
    image: false,
    audio: false,
    embedded_context: false,
};

#[test]
fn parses_text_and_files_from_a_send_body() {
    let prompt = Prompt::from_body(&body_with(vec![
        json!({ "type": "text", "text": "what is this" }),
        file_part("image/png", "shot.png", PNG),
    ]));

    assert_eq!(prompt.text, "what is this");
    assert_eq!(prompt.attachments.len(), 1);
    assert_eq!(prompt.attachments[0].filename, "shot.png");
    assert_eq!(prompt.attachments[0].mime, "image/png");
    assert_eq!(prompt.attachments[0].data, PNG);
}

/// The composer omits the text part when the user attaches without typing. That is still a
/// turn, and treating it as empty is how an upload silently does nothing.
#[test]
fn an_attachment_alone_is_not_an_empty_prompt() {
    let prompt = Prompt::from_body(&body_with(vec![file_part("image/png", "shot.png", PNG)]));

    assert!(!prompt.is_empty());
    assert_eq!(prompt.text, "");
    assert_eq!(prompt.attachments.len(), 1);
}

#[test]
fn a_prompt_with_neither_text_nor_files_is_empty() {
    assert!(Prompt::from_body(&body_with(vec![])).is_empty());
    assert!(Prompt::text("   ").is_empty());
}

/// Bodies that predate `parts[]`, and the slash-command path.
#[test]
fn falls_back_to_a_bare_text_field() {
    assert_eq!(Prompt::from_body(&json!({ "text": "hello" })).text, "hello");
    assert_eq!(Prompt::from_body(&json!({ "prompt": "hi" })).text, "hi");
}

/// A file part with no data URL carries nothing opman can forward, so it must not become an
/// attachment that claims otherwise.
#[test]
fn a_file_part_without_a_data_url_is_skipped() {
    let prompt = Prompt::from_body(&body_with(vec![
        json!({ "type": "text", "text": "hi" }),
        json!({ "type": "file", "mime": "image/png", "filename": "x.png" }),
    ]));

    assert!(prompt.attachments.is_empty());
}

#[test]
fn an_image_goes_inline_when_the_agent_takes_images() {
    let prompt = Prompt::from_body(&body_with(vec![file_part("image/png", "shot.png", PNG)]));
    let blocks = prompt.content_blocks(ALL);

    assert_eq!(blocks[0]["type"], "text");
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(blocks[1]["mimeType"], "image/png");
    assert_eq!(blocks[1]["data"], PNG);
}

/// The whole point of gating on `promptCapabilities`: an agent that never advertised images
/// must not be handed one, but must still be told a file exists.
#[test]
fn an_image_degrades_to_a_link_when_the_agent_takes_none() {
    let prompt = Prompt::from_body(&body_with(vec![file_part("image/png", "shot.png", PNG)]));
    let blocks = prompt.content_blocks(NONE);

    assert_eq!(blocks[1]["type"], "resource_link");
    assert_eq!(blocks[1]["name"], "shot.png");
    assert_eq!(blocks[1]["uri"], "file:///shot.png");
}

/// Text-ish uploads are inlined decoded, which is what makes an attached log readable to the
/// agent instead of a name it would have to go and fetch.
#[test]
fn a_text_file_is_embedded_decoded() {
    use base64::Engine as _;
    let data = BASE64.encode("line one\nline two");
    let prompt = Prompt::from_body(&body_with(vec![file_part("text/plain", "log.txt", &data)]));
    let blocks = prompt.content_blocks(ALL);

    assert_eq!(blocks[1]["type"], "resource");
    assert_eq!(blocks[1]["resource"]["text"], "line one\nline two");
    assert_eq!(blocks[1]["resource"]["uri"], "file:///log.txt");
}

/// Bytes that are not text must travel as a blob rather than being mangled into one.
#[test]
fn a_binary_file_is_embedded_as_a_blob() {
    let prompt = Prompt::from_body(&body_with(vec![file_part(
        "application/pdf",
        "spec.pdf",
        PNG,
    )]));
    let blocks = prompt.content_blocks(ALL);

    assert_eq!(blocks[1]["type"], "resource");
    assert_eq!(blocks[1]["resource"]["blob"], PNG);
    assert!(blocks[1]["resource"]["text"].is_null());
}

/// The text block is always present. An agent handed only an image still needs something to
/// respond to, and some reject a prompt with no text block at all.
#[test]
fn the_text_block_is_always_first_even_when_empty() {
    let prompt = Prompt::from_body(&body_with(vec![file_part("image/png", "a.png", PNG)]));
    let blocks = prompt.content_blocks(ALL);

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0]["type"], "text");
    assert_eq!(blocks[0]["text"], "");
}

/// The user's own bubble has to show what they attached, which is the preview half of this
/// feature: a `file` part with the original data URL is what the timeline renders as an image.
#[test]
fn message_parts_carry_the_attachment_for_preview() {
    let prompt = Prompt::from_body(&body_with(vec![
        json!({ "type": "text", "text": "look" }),
        file_part("image/png", "shot.png", PNG),
    ]));
    let parts = prompt.message_parts("msg_user_1", "ses_a");

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["type"], "text");
    assert_eq!(parts[0]["text"], "look");
    assert_eq!(parts[1]["type"], "file");
    assert_eq!(parts[1]["mime"], "image/png");
    assert_eq!(parts[1]["filename"], "shot.png");
    // A data URL, because the renderer puts it straight into an `<img src>`.
    assert_eq!(
        parts[1]["url"],
        format!("data:image/png;base64,{PNG}")
    );
    // Ids stay unique within the message so the part map cannot collapse them.
    assert_eq!(parts[0]["id"], "msg_user_1:0");
    assert_eq!(parts[1]["id"], "msg_user_1:1");
    assert_eq!(parts[1]["messageID"], "msg_user_1");
    assert_eq!(parts[1]["sessionID"], "ses_a");
}

#[test]
fn several_attachments_keep_their_order_and_ids() {
    let prompt = Prompt::from_body(&body_with(vec![
        json!({ "type": "text", "text": "two files" }),
        file_part("image/png", "one.png", PNG),
        file_part("image/jpeg", "two.jpg", PNG),
    ]));
    let parts = prompt.message_parts("m1", "s1");

    assert_eq!(parts.len(), 3);
    assert_eq!(parts[1]["filename"], "one.png");
    assert_eq!(parts[2]["filename"], "two.jpg");
    assert_eq!(parts[2]["id"], "m1:2");
}

/// Absent capability fields mean unsupported — the spec's default, and the safe reading for
/// an agent that predates the field.
#[test]
fn absent_prompt_capabilities_read_as_unsupported() {
    assert_eq!(PromptCaps::from_initialize(&json!({})), NONE);
    let claude = json!({
        "agentCapabilities": { "promptCapabilities": { "image": true, "embeddedContext": true } }
    });
    let caps = PromptCaps::from_initialize(&claude);
    assert!(caps.image);
    assert!(caps.embedded_context);
    assert!(!caps.audio);
}
