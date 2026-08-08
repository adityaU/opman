//! Document-sync tests. The contract: open once, change only when the text
//! really moved, and never send a stale version number.

use super::*;
use crate::lsp::framing::read_frame;
use crate::lsp::peer::Handler;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::BufReader;

struct Silent;
impl Handler for Silent {
    fn request(&self, _method: &str, _params: &Value) -> anyhow::Result<Value> {
        Ok(Value::Null)
    }
    fn notify(&self, _method: &str, _params: Value) {}
}

/// A peer wired to a pipe we can read the outgoing frames from.
fn wired() -> (
    Peer,
    BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
) {
    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let peer = Peer::new(ours, Arc::new(Silent));
    let (read, _write) = tokio::io::split(theirs);
    (peer, BufReader::new(read))
}

async fn next_frame(reader: &mut BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(reader))
        .await
        .expect("a frame should arrive")
        .expect("frame decodes")
        .expect("not EOF")
}

#[tokio::test]
async fn first_sync_opens_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let (peer, mut reader) = wired();
    let docs = DocStore::new();
    docs.sync(&peer, &file, "rust", None).unwrap();

    let frame = next_frame(&mut reader).await;
    assert_eq!(frame["method"], "textDocument/didOpen");
    assert_eq!(frame["params"]["textDocument"]["languageId"], "rust");
    assert_eq!(frame["params"]["textDocument"]["version"], 1);
    assert_eq!(frame["params"]["textDocument"]["text"], "fn main() {}");
}

/// The hot path: hovering repeatedly must not re-send the document each time.
#[tokio::test]
async fn unchanged_content_sends_nothing_further() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let (peer, mut reader) = wired();
    let docs = DocStore::new();
    docs.sync(&peer, &file, "rust", Some("fn main() {}"))
        .unwrap();
    let _open = next_frame(&mut reader).await;

    docs.sync(&peer, &file, "rust", Some("fn main() {}"))
        .unwrap();
    docs.sync(&peer, &file, "rust", Some("fn main() {}"))
        .unwrap();

    // Force a frame we can recognise; if a didChange had been sent it would
    // arrive before this one.
    peer.notify("sentinel", serde_json::json!({})).unwrap();
    let frame = next_frame(&mut reader).await;
    assert_eq!(frame["method"], "sentinel", "no redundant didChange");
}

#[tokio::test]
async fn edited_content_sends_a_change_with_the_next_version() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let (peer, mut reader) = wired();
    let docs = DocStore::new();
    docs.sync(&peer, &file, "rust", Some("fn main() {}"))
        .unwrap();
    let _open = next_frame(&mut reader).await;

    docs.sync(&peer, &file, "rust", Some("fn main() { todo!() }"))
        .unwrap();
    let frame = next_frame(&mut reader).await;
    assert_eq!(frame["method"], "textDocument/didChange");
    assert_eq!(frame["params"]["textDocument"]["version"], 2);
    assert_eq!(
        frame["params"]["contentChanges"][0]["text"],
        "fn main() { todo!() }"
    );
}

/// Versions must keep climbing; a repeated version makes servers discard edits.
#[tokio::test]
async fn versions_increase_monotonically() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "a").unwrap();

    let (peer, mut reader) = wired();
    let docs = DocStore::new();
    docs.sync(&peer, &file, "rust", Some("a")).unwrap();
    assert_eq!(
        next_frame(&mut reader).await["params"]["textDocument"]["version"],
        1
    );

    for (index, text) in ["b", "c", "d"].iter().enumerate() {
        docs.sync(&peer, &file, "rust", Some(text)).unwrap();
        let frame = next_frame(&mut reader).await;
        assert_eq!(frame["params"]["textDocument"]["version"], index as i64 + 2);
    }
}

/// After a respawn the server has no documents, so ours must reopen at v1.
#[tokio::test]
async fn clearing_reopens_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "x").unwrap();

    let (peer, mut reader) = wired();
    let docs = DocStore::new();
    docs.sync(&peer, &file, "rust", Some("x")).unwrap();
    let _open = next_frame(&mut reader).await;

    docs.clear();
    assert!(!docs.is_open(&crate::lsp::convert::path_to_uri(&file)));

    docs.sync(&peer, &file, "rust", Some("x")).unwrap();
    let frame = next_frame(&mut reader).await;
    assert_eq!(frame["method"], "textDocument/didOpen");
    assert_eq!(frame["params"]["textDocument"]["version"], 1);
}

#[tokio::test]
async fn a_missing_file_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let (peer, _reader) = wired();
    let docs = DocStore::new();
    assert!(docs
        .sync(&peer, &dir.path().join("gone.rs"), "rust", None)
        .is_err());
}
