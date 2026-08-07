//! Framing is the layer where a mistake corrupts everything downstream, so it
//! is tested against the exact byte sequences servers emit.

use super::*;
use serde_json::json;
use tokio::io::BufReader;

async fn decode(bytes: &[u8]) -> Result<Option<Value>> {
    let mut reader = BufReader::new(bytes);
    read_frame(&mut reader).await
}

#[tokio::test]
async fn reads_a_well_formed_frame() {
    let frame = decode(b"Content-Length: 17\r\n\r\n{\"jsonrpc\":\"2.0\"}")
        .await
        .unwrap()
        .expect("a frame");
    assert_eq!(frame["jsonrpc"], "2.0");
}

/// Servers are free to send Content-Type; it carries nothing we need.
#[tokio::test]
async fn ignores_other_headers() {
    let raw = b"Content-Length: 17\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{\"jsonrpc\":\"2.0\"}";
    let frame = decode(raw).await.unwrap().expect("a frame");
    assert_eq!(frame["jsonrpc"], "2.0");
}

#[tokio::test]
async fn clean_eof_is_not_an_error() {
    assert!(decode(b"").await.unwrap().is_none());
}

/// Truncation mid-header means the server died; that is an error, not an EOF,
/// because a caller waiting on a response needs to be told.
#[tokio::test]
async fn truncated_header_is_an_error() {
    assert!(decode(b"Content-Length: 12").await.is_err());
}

#[tokio::test]
async fn missing_content_length_is_an_error() {
    assert!(decode(b"Content-Type: x\r\n\r\n{}").await.is_err());
}

#[tokio::test]
async fn malformed_header_is_an_error() {
    assert!(decode(b"not-a-header\r\n\r\n{}").await.is_err());
}

#[tokio::test]
async fn absurd_length_is_refused_without_allocating() {
    let raw = format!("Content-Length: {}\r\n\r\n", u32::MAX);
    assert!(decode(raw.as_bytes()).await.is_err());
}

/// The length is a byte count, not a character count — a multi-byte payload
/// read as characters would desynchronise the stream permanently.
#[tokio::test]
async fn length_counts_bytes_not_characters() {
    let mut buffer = Vec::new();
    write_frame(&mut buffer, &json!({ "s": "héllo — ok" }))
        .await
        .unwrap();
    let frame = decode(&buffer).await.unwrap().expect("a frame");
    assert_eq!(frame["s"], "héllo — ok");
}

#[tokio::test]
async fn round_trips_back_to_back_frames() {
    let mut buffer = Vec::new();
    write_frame(&mut buffer, &json!({ "n": 1 })).await.unwrap();
    write_frame(&mut buffer, &json!({ "n": 2 })).await.unwrap();

    let mut reader = BufReader::new(&buffer[..]);
    assert_eq!(read_frame(&mut reader).await.unwrap().unwrap()["n"], 1);
    assert_eq!(read_frame(&mut reader).await.unwrap().unwrap()["n"], 2);
    assert!(read_frame(&mut reader).await.unwrap().is_none());
}
