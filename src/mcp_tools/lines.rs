//! Read one line at a time without letting a single huge line fill memory.

use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

/// Reads the next line into `out` (without its line ending), keeping at most `cap` bytes.
/// Returns `None` at the end of the input, else whether the line was cut short.
pub async fn read_line_capped<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    out: &mut Vec<u8>,
    cap: usize,
) -> io::Result<Option<bool>> {
    out.clear();
    let mut clipped = false;
    let mut seen_any = false;
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            return Ok(seen_any.then_some(clipped));
        }
        seen_any = true;
        let newline = chunk.iter().position(|&b| b == b'\n');
        let used = newline.map_or(chunk.len(), |i| i + 1);
        let body = newline.unwrap_or(chunk.len());
        let take = body.min(cap.saturating_sub(out.len()));
        clipped |= take < body;
        out.extend_from_slice(&chunk[..take]);
        reader.consume(used);
        if newline.is_some() {
            break;
        }
    }
    if out.last() == Some(&b'\r') {
        out.pop();
    }
    Ok(Some(clipped))
}
