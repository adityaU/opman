use super::*;

use serde_json::json;

#[test]
fn text_blocks_stay_text() {
    assert_eq!(
        render(&json!({ "type": "text", "text": "hello" })),
        Some(Rendered::Text("hello".to_string()))
    );
}

/// An empty chunk is not a part; agents send them as keep-alives between tokens.
#[test]
fn empty_text_renders_nothing() {
    assert_eq!(render(&json!({ "type": "text", "text": "" })), None);
}

/// The gap this module exists to close: an image reply used to render as a blank message
/// because only `content.text` was read.
#[test]
fn images_become_file_parts() {
    let block = json!({ "type": "image", "mimeType": "image/png", "data": "AAAA" });
    assert_eq!(
        render(&block),
        Some(Rendered::File {
            mime: "image/png".to_string(),
            filename: "image/png".to_string(),
            url: "data:image/png;base64,AAAA".to_string(),
        })
    );
}

/// A URI names the file; an inline image has none and falls back to its media type.
#[test]
fn a_uri_names_the_file() {
    let block = json!({
        "type": "image", "mimeType": "image/png", "data": "AAAA",
        "uri": "file:///tmp/plot.png",
    });
    let Some(Rendered::File { filename, .. }) = render(&block) else {
        panic!("expected a file part");
    };
    assert_eq!(filename, "plot.png");
}

/// A link is a name and a place. Rendering it as a link is the whole of what the agent said,
/// and keeps the path clickable in the timeline.
#[test]
fn resource_links_render_as_markdown_links() {
    let block = json!({ "type": "resource_link", "uri": "file:///a/b.rs", "name": "b.rs" });
    assert_eq!(
        render(&block),
        Some(Rendered::Text("[b.rs](file:///a/b.rs)".to_string()))
    );
}

#[test]
fn a_link_without_a_name_shows_its_uri() {
    let block = json!({ "type": "resource_link", "uri": "file:///a/b.rs" });
    assert_eq!(
        render(&block),
        Some(Rendered::Text(
            "[file:///a/b.rs](file:///a/b.rs)".to_string()
        ))
    );
}

#[test]
fn embedded_text_resources_render_as_their_text() {
    let block = json!({
        "type": "resource",
        "resource": { "uri": "file:///a.txt", "mimeType": "text/plain", "text": "inline" },
    });
    assert_eq!(render(&block), Some(Rendered::Text("inline".to_string())));
}

/// A resource carrying bytes rather than text is a file, named by the URI it arrived with.
#[test]
fn embedded_blobs_become_file_parts() {
    let block = json!({
        "type": "resource",
        "resource": { "uri": "file:///a/sound.wav", "mimeType": "audio/wav", "blob": "QUJD" },
    });
    let Some(Rendered::File {
        filename,
        mime,
        url,
    }) = render(&block)
    else {
        panic!("expected a file part");
    };
    assert_eq!(filename, "sound.wav");
    assert_eq!(mime, "audio/wav");
    assert_eq!(url, "data:audio/wav;base64,QUJD");
}

/// The protocol is versioned and additive: a block type opman has never heard of must be
/// skipped, not treated as an error that breaks the message around it.
#[test]
fn unknown_block_types_are_ignored() {
    assert_eq!(render(&json!({ "type": "hologram", "data": "x" })), None);
    assert_eq!(render(&json!({})), None);
}
