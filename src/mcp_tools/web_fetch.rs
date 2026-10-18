//! `web_fetch`: download an http(s) page and return it as readable text.

use super::html;
use super::limits::{ByteLimit, Timeout};
use super::output::{ToolError, ToolOutput, ToolResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_BYTES: ByteLimit = ByteLimit::new(200_000);
const MAX_BYTES: ByteLimit = ByteLimit::new(2_000_000);
const TIMEOUT: Timeout = Timeout::from_secs(30);
const USER_AGENT: &str = concat!("opman-tools/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
pub struct FetchArgs {
    url: String,
    max_bytes: Option<u64>,
}

#[derive(Serialize)]
struct FetchData {
    url: String,
    status: u16,
    content_type: String,
    truncated: bool,
    content: String,
}

pub async fn run(args: FetchArgs) -> ToolResult {
    let url = reqwest::Url::parse(args.url.trim())
        .map_err(|e| ToolError::new(format!("invalid url: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ToolError::new(format!(
            "only http and https urls can be fetched, not '{}'",
            url.scheme()
        )));
    }
    let limit = ByteLimit::requested(args.max_bytes, DEFAULT_BYTES, MAX_BYTES);
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT.get())
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| ToolError::new(format!("cannot start the http client: {e}")))?;
    let mut response = client
        .get(url)
        .header("Accept", "text/html,text/plain,application/json,*/*;q=0.5")
        .send()
        .await
        .map_err(|e| ToolError::new(format!("request failed: {e}")))?;

    let status = response.status();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if is_binary(&content_type) {
        return Err(ToolError::new(format!(
            "{final_url} is {content_type}, which is not text"
        )));
    }

    // Stop reading at the limit: a huge page never fills memory.
    let mut body: Vec<u8> = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| ToolError::new(format!("download failed: {e}")))?
    {
        let room = limit.get() - body.len();
        if chunk.len() > room {
            body.extend_from_slice(&chunk[..room]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }
    let raw = String::from_utf8_lossy(&body);
    let is_html = content_type.contains("html") || looks_like_html(&raw);
    let content = if is_html {
        html::to_text(&raw)
    } else {
        raw.into_owned()
    };

    let mut text = format!("{final_url} (HTTP {})\n\n{content}", status.as_u16());
    if truncated {
        text.push_str(&format!(
            "\n\n[stopped after {} bytes; raise max_bytes to read more]",
            limit.get()
        ));
    }
    let data = FetchData {
        url: final_url,
        status: status.as_u16(),
        content_type,
        truncated,
        content,
    };
    if status.is_client_error() || status.is_server_error() {
        return ToolOutput::failed(text, &data);
    }
    ToolOutput::ok(text, &data)
}

fn looks_like_html(text: &str) -> bool {
    let start: String = text
        .trim_start()
        .chars()
        .take(15)
        .collect::<String>()
        .to_ascii_lowercase();
    start.starts_with("<!doctype html") || start.starts_with("<html")
}

fn is_binary(content_type: &str) -> bool {
    [
        "image/",
        "audio/",
        "video/",
        "application/zip",
        "application/octet-stream",
        "application/pdf",
        "font/",
    ]
    .iter()
    .any(|prefix| content_type.starts_with(prefix))
}
