//! HTTP fetch wrappers with cookie-based auth.
//! All requests go to `/api/...` with `credentials: same-origin`.

use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestCredentials, Response};

/// API error type.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API error {}: {}", self.status, self.message)
    }
}

/// Clear any legacy token from sessionStorage.
pub fn clear_token() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.session_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("opman_token");
    }
}

/// Handle 401 by clearing token and reloading.
fn handle_unauthorized() {
    clear_token();
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

/// Internal fetch helper.
///
/// When `handle_401` is true (the default path), a 401 response triggers
/// `handle_unauthorized()` which clears the legacy token and reloads the
/// page.  Auth-related endpoints (verify, login) set `handle_401 = false`
/// so that a 401 is returned as a normal `ApiError` instead of causing an
/// infinite reload loop.
async fn do_fetch(
    method: &str,
    path: &str,
    body: Option<String>,
    handle_401: bool,
) -> Result<Response, ApiError> {
    let url = format!("/api{}", path);
    let mut opts = RequestInit::new();
    opts.set_method(method);
    opts.set_credentials(RequestCredentials::SameOrigin);

    if let Some(ref b) = body {
        let js_body = JsValue::from_str(b);
        opts.set_body(&js_body);
    }

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| ApiError { status: 0, message: format!("Request creation failed: {:?}", e) })?;

    // Set headers
    let headers = request.headers();
    let _ = headers.set("Content-Type", "application/json");

    let window = web_sys::window()
        .ok_or_else(|| ApiError { status: 0, message: "No window".into() })?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| ApiError { status: 0, message: format!("Fetch failed: {:?}", e) })?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| ApiError { status: 0, message: "Response cast failed".into() })?;

    let status = resp.status();

    if status == 401 {
        if handle_401 {
            handle_unauthorized();
        }
        return Err(ApiError { status: 401, message: "Unauthorized".into() });
    }

    if status < 200 || status >= 300 {
        let text = JsFuture::from(resp.text().map_err(|_| ApiError { status, message: "Failed to read error body".into() })?)
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        return Err(ApiError { status, message: format!("{} {}", status, text) });
    }

    Ok(resp)
}

/// GET request, returning deserialized JSON.
pub async fn api_fetch<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let resp = do_fetch("GET", path, None, true).await?;
    let text = JsFuture::from(resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?)
        .await
        .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
        .as_string()
        .unwrap_or_default();
    serde_json::from_str(&text)
        .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
}

/// POST request with optional JSON body.
pub async fn api_post<T: DeserializeOwned>(path: &str, body: &impl Serialize) -> Result<T, ApiError> {
    let json = serde_json::to_string(body)
        .map_err(|e| ApiError { status: 0, message: format!("Serialize error: {}", e) })?;
    let resp = do_fetch("POST", path, Some(json), true).await?;
    let text = JsFuture::from(resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?)
        .await
        .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
        .as_string()
        .unwrap_or_default();
    if text.is_empty() {
        // Return default for void responses
        serde_json::from_str("null")
            .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
    } else {
        serde_json::from_str(&text)
            .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
    }
}

/// POST request with no return value.
pub async fn api_post_void(path: &str, body: &impl Serialize) -> Result<(), ApiError> {
    let json = serde_json::to_string(body)
        .map_err(|e| ApiError { status: 0, message: format!("Serialize error: {}", e) })?;
    let _resp = do_fetch("POST", path, Some(json), true).await?;
    Ok(())
}

/// DELETE request.
pub async fn api_delete(path: &str) -> Result<(), ApiError> {
    let _resp = do_fetch("DELETE", path, None, true).await?;
    Ok(())
}

/// DELETE request with JSON body.
pub async fn api_delete_with_body(path: &str, body: &impl Serialize) -> Result<(), ApiError> {
    let json = serde_json::to_string(body)
        .map_err(|e| ApiError { status: 0, message: format!("Serialize error: {}", e) })?;
    let _resp = do_fetch("DELETE", path, Some(json), true).await?;
    Ok(())
}

/// PATCH request with JSON body.
pub async fn api_patch<T: DeserializeOwned>(path: &str, body: &impl Serialize) -> Result<T, ApiError> {
    let json = serde_json::to_string(body)
        .map_err(|e| ApiError { status: 0, message: format!("Serialize error: {}", e) })?;
    let resp = do_fetch("PATCH", path, Some(json), true).await?;
    let text = JsFuture::from(resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?)
        .await
        .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
        .as_string()
        .unwrap_or_default();
    if text.is_empty() {
        serde_json::from_str("null")
            .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
    } else {
        serde_json::from_str(&text)
            .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
    }
}

/// PUT request with JSON body.
pub async fn api_put<T: DeserializeOwned>(path: &str, body: &impl Serialize) -> Result<T, ApiError> {
    let json = serde_json::to_string(body)
        .map_err(|e| ApiError { status: 0, message: format!("Serialize error: {}", e) })?;
    let resp = do_fetch("PUT", path, Some(json), true).await?;
    let text = JsFuture::from(resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?)
        .await
        .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
        .as_string()
        .unwrap_or_default();
    if text.is_empty() {
        serde_json::from_str("null")
            .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
    } else {
        serde_json::from_str(&text)
            .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
    }
}

/// POST to login endpoint (unauthenticated).
///
/// Uses `handle_401 = false` so that invalid credentials return an error
/// instead of triggering the global page-reload behavior.
pub async fn login(username: &str, password: &str) -> Result<String, ApiError> {
    #[derive(Serialize)]
    struct LoginBody<'a> {
        username: &'a str,
        password: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct LoginResponse {
        token: String,
    }
    let json = serde_json::to_string(&LoginBody { username, password })
        .map_err(|e| ApiError { status: 0, message: format!("Serialize error: {}", e) })?;
    let resp = do_fetch("POST", "/auth/login", Some(json), false).await?;
    let text = JsFuture::from(resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?)
        .await
        .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
        .as_string()
        .unwrap_or_default();
    let parsed: LoginResponse = serde_json::from_str(&text)
        .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })?;
    Ok(parsed.token)
}

/// Verify current auth (cookie-based).
///
/// Uses `handle_401 = false` so an unauthenticated state returns `false`
/// instead of triggering `handle_unauthorized()` (which would reload the
/// page and cause an infinite loop).
pub async fn verify_token() -> bool {
    match do_fetch("GET", "/auth/verify", None, false).await {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Fetch public bootstrap data (no auth required).
pub async fn fetch_bootstrap() -> Result<crate::types::api::BootstrapData, ApiError> {
    // Bootstrap is a public endpoint, use raw fetch without /api prefix requirement
    let resp = do_fetch("GET", "/public/bootstrap", None, false).await?;
    let text = JsFuture::from(resp.text().map_err(|_| ApiError { status: 0, message: "text() failed".into() })?)
        .await
        .map_err(|_| ApiError { status: 0, message: "text() promise failed".into() })?
        .as_string()
        .unwrap_or_default();
    serde_json::from_str(&text)
        .map_err(|e| ApiError { status: 0, message: format!("JSON parse error: {}", e) })
}

// ── Session message & stats fetchers ────────────────────────────────

/// Fetch messages for a session with pagination.
pub async fn fetch_session_messages(
    session_id: &str,
    limit: usize,
    before: Option<f64>,
) -> Result<crate::types::api::MessagePageResponse, ApiError> {
    let mut path = format!(
        "/session/{}/messages?limit={}",
        js_sys::encode_uri_component(session_id),
        limit,
    );
    if let Some(b) = before {
        path.push_str(&format!("&before={}", b));
    }
    api_fetch(&path).await
}

/// Fetch stats for a session.
pub async fn fetch_session_stats(
    session_id: &str,
) -> Result<crate::types::api::SessionStats, ApiError> {
    let path = format!(
        "/session/{}/stats",
        js_sys::encode_uri_component(session_id),
    );
    api_fetch(&path).await
}

/// Send a message to a session.
/// Body must match React format: `{ parts: [{type:"text", text:...}, ...], model?, agent? }`.
pub async fn send_message(
    session_id: &str,
    content: &str,
    images: Option<Vec<String>>,
) -> Result<serde_json::Value, ApiError> {
    let mut parts = vec![serde_json::json!({ "type": "text", "text": content })];
    if let Some(ref imgs) = images {
        for data_url in imgs {
            // Parse data URL: "data:<mime>;base64,<data>"
            let (mime, base64) = if let Some(rest) = data_url.strip_prefix("data:") {
                if let Some((mime_part, b64_part)) = rest.split_once(";base64,") {
                    (mime_part.to_string(), b64_part.to_string())
                } else {
                    ("image/png".to_string(), rest.to_string())
                }
            } else {
                ("image/png".to_string(), data_url.clone())
            };
            parts.push(serde_json::json!({
                "type": "image",
                "image": base64,
                "mimeType": mime,
            }));
        }
    }
    let body = serde_json::json!({ "parts": parts });
    let path = format!(
        "/session/{}/message",
        js_sys::encode_uri_component(session_id),
    );
    api_post(&path, &body).await
}

/// Abort a running session.
pub async fn abort_session(session_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/session/{}/abort",
        js_sys::encode_uri_component(session_id),
    );
    api_post_void(&path, &serde_json::json!({})).await
}

/// Fetch pending permissions and questions.
pub async fn fetch_pending() -> Result<crate::types::api::PendingResponse, ApiError> {
    api_fetch("/pending").await
}

/// Reply to a permission request.
pub async fn reply_permission(request_id: &str, reply: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        reply: &'a str,
    }
    let path = format!(
        "/permission/{}/reply",
        js_sys::encode_uri_component(request_id),
    );
    api_post_void(&path, &Body { reply }).await
}

/// Reply to a question request with answers.
pub async fn reply_question(request_id: &str, answers: &[Vec<String>]) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        answers: &'a [Vec<String>],
    }
    let path = format!(
        "/question/{}/reply",
        js_sys::encode_uri_component(request_id),
    );
    api_post_void(&path, &Body { answers }).await
}

/// Reject/dismiss a question request.
pub async fn reject_question(request_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/question/{}/reject",
        js_sys::encode_uri_component(request_id),
    );
    api_post_void(&path, &serde_json::json!({})).await
}

/// Fetch theme colors.
pub async fn fetch_theme() -> Result<Option<crate::types::api::ThemeColors>, ApiError> {
    match api_fetch::<crate::types::api::ThemeColors>("/theme").await {
        Ok(colors) => Ok(Some(colors)),
        Err(e) if e.status == 404 => Ok(None),
        Err(e) => Err(e),
    }
}
