//! Raw file bytes with HTTP `Range` support, so `<video>` and `<audio>` can seek.

use axum::body::{Body, Bytes};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use std::io::SeekFrom;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::super::error::{WebError, WebResult};

/// One answer never carries more than this, so a huge video is served in chunks.
const MAX_CHUNK: u64 = 8 * 1024 * 1024;

/// An inclusive byte range that is known to sit inside the file.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) struct ByteRange {
    pub start: u64,
    pub end: u64,
}

/// What a `Range` header asks for.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RangeAsk {
    /// No header, or one we do not understand: send the whole file.
    Whole,
    Slice(ByteRange),
    /// The range starts past the end of the file.
    Unsatisfiable,
}

/// Read a single `bytes=a-b`, `bytes=a-` or `bytes=-n` range. Several ranges are ignored.
pub(super) fn parse_range(header: Option<&str>, len: u64) -> RangeAsk {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return RangeAsk::Whole;
    };
    if spec.contains(',') {
        return RangeAsk::Whole;
    }
    let Some((from, to)) = spec.split_once('-') else {
        return RangeAsk::Whole;
    };
    let (from, to) = (from.trim(), to.trim());
    let last = len.saturating_sub(1);
    let (start, end) = if from.is_empty() {
        // Suffix form: the last `n` bytes.
        let Ok(n) = to.parse::<u64>() else {
            return RangeAsk::Whole;
        };
        (len.saturating_sub(n), last)
    } else {
        let Ok(start) = from.parse::<u64>() else {
            return RangeAsk::Whole;
        };
        let end = if to.is_empty() {
            last
        } else {
            match to.parse::<u64>() {
                Ok(e) => e.min(last),
                Err(_) => return RangeAsk::Whole,
            }
        };
        (start, end)
    };
    if len == 0 || start >= len || start > end {
        return RangeAsk::Unsatisfiable;
    }
    let end = end.min(start.saturating_add(MAX_CHUNK - 1));
    RangeAsk::Slice(ByteRange { start, end })
}

/// Serve `path` (already checked by the caller) as `content_type`, honouring `Range`.
pub(super) async fn serve_file(
    path: &Path,
    content_type: String,
    headers: &HeaderMap,
) -> WebResult<Response> {
    let io = |e: std::io::Error| WebError::Internal(format!("Failed to read file: {e}"));
    let mut file = tokio::fs::File::open(path).await.map_err(io)?;
    let meta = file.metadata().await.map_err(io)?;
    if !meta.is_file() {
        return Err(WebError::Internal("Failed to read file: not a file".into()));
    }
    let len = meta.len();
    let ask = parse_range(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        len,
    );
    let ctype = HeaderValue::from_str(&content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

    let slice = match ask {
        RangeAsk::Unsatisfiable => {
            let total = HeaderValue::from_str(&format!("bytes */{len}"))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0"));
            return Ok((
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, total)],
            )
                .into_response());
        }
        RangeAsk::Whole => None,
        RangeAsk::Slice(r) => Some(r),
    };

    let (start, count) = slice
        .as_ref()
        .map_or((0, len), |r| (r.start, r.end - r.start + 1));
    file.seek(SeekFrom::Start(start)).await.map_err(io)?;
    let stream = async_stream::try_stream! {
        let mut remaining = count;
        while remaining > 0 {
            let mut buffer = vec![0; remaining.min(64 * 1024) as usize];
            let read = file.read(&mut buffer).await?;
            if read == 0 { break; }
            buffer.truncate(read);
            remaining -= read as u64;
            yield Bytes::from(buffer);
        }
    };
    let stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>,
    > = Box::pin(stream);
    let mut resp = Body::from_stream(stream).into_response();
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, ctype);
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(length) = HeaderValue::from_str(&count.to_string()) {
        h.insert(header::CONTENT_LENGTH, length);
    }
    if let Some(r) = slice {
        *resp.status_mut() = StatusCode::PARTIAL_CONTENT;
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {}-{}/{}", r.start, r.end, len)) {
            resp.headers_mut().insert(header::CONTENT_RANGE, v);
        }
    }
    Ok(resp)
}

#[cfg(test)]
#[path = "raw_range_tests.rs"]
mod raw_range_tests;
