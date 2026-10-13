//! A stand-in for both usage endpoints. The bearer token picks the behaviour, so one
//! server serves every test case:
//! `limits` / `legacy` (Claude shapes), `codex` (ChatGPT shape), `unauthorized` (401),
//! `slow` (never answers in time), `boom` (500).

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use super::claude::ClaudeSource;
use super::codex::CodexSource;
use crate::web::test_support::start_mock_upstream;

#[derive(Clone, Default)]
pub struct Fake {
    pub claude_hits: Arc<AtomicUsize>,
    pub codex_hits: Arc<AtomicUsize>,
}

impl Fake {
    pub fn claude_hits(&self) -> usize {
        self.claude_hits.load(Ordering::SeqCst)
    }

    pub fn codex_hits(&self) -> usize {
        self.codex_hits.load(Ordering::SeqCst)
    }
}

pub async fn start() -> (String, Fake) {
    let fake = Fake::default();
    let router = Router::new()
        .route("/api/oauth/usage", get(claude))
        .route("/backend-api/wham/usage", get(codex))
        .with_state(fake.clone());
    (start_mock_upstream(router).await, fake)
}

fn token(headers: &HeaderMap) -> &str {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("")
}

async fn common(token: &str) -> Option<Response> {
    match token {
        "unauthorized" => Some(StatusCode::UNAUTHORIZED.into_response()),
        "boom" => Some(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        "slow" => {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Some(StatusCode::OK.into_response())
        }
        _ => None,
    }
}

async fn claude(State(fake): State<Fake>, headers: HeaderMap) -> Response {
    fake.claude_hits.fetch_add(1, Ordering::SeqCst);
    if headers
        .get("anthropic-beta")
        .is_none_or(|v| v != "oauth-2025-04-20")
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let token = token(&headers).to_string();
    if let Some(response) = common(&token).await {
        return response;
    }
    match token.as_str() {
        "legacy" => Json(legacy_body()).into_response(),
        _ => Json(limits_body()).into_response(),
    }
}

async fn codex(State(fake): State<Fake>, headers: HeaderMap) -> Response {
    fake.codex_hits.fetch_add(1, Ordering::SeqCst);
    let agent_ok = headers
        .get("user-agent")
        .is_some_and(|v| v == "codex_cli_rs");
    let account_ok = headers
        .get("chatgpt-account-id")
        .is_some_and(|v| v == "acct-1");
    if !agent_ok || !account_ok {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let token = token(&headers).to_string();
    if let Some(response) = common(&token).await {
        return response;
    }
    Json(codex_body()).into_response()
}

pub fn limits_body() -> Value {
    json!({
        "five_hour": { "utilization": 1.0, "resets_at": "2026-10-12T10:20:00.434102+00:00" },
        "seven_day": { "utilization": 2.0, "resets_at": "2026-10-16T11:00:00+00:00" },
        "seven_day_opus": null,
        "extra_usage": { "is_enabled": true, "monthly_limit": 5000, "used_credits": 1250, "utilization": 25.0 },
        "limits": [
            { "kind": "session", "group": "session", "percent": 76, "severity": "warning",
              "resets_at": "2026-10-12T10:20:00.434102+00:00", "is_active": true },
            { "kind": "weekly_all", "group": "weekly", "percent": 15, "severity": "normal",
              "resets_at": "2026-10-16T11:00:00.434125+00:00", "is_active": false },
            { "kind": "weekly_opus", "group": "weekly", "percent": 95, "severity": "mystery",
              "resets_at": null, "is_active": false }
        ]
    })
}

pub fn legacy_body() -> Value {
    json!({
        "five_hour": { "utilization": 76.0, "resets_at": "2026-10-12T10:20:00.434102+00:00" },
        "seven_day": { "utilization": 15.0, "resets_at": "2026-10-16T11:00:00.434125+00:00" },
        "seven_day_opus": { "utilization": 80.0, "resets_at": "not a date" },
        "seven_day_sonnet": null,
        "extra_usage": { "is_enabled": false, "monthly_limit": null, "used_credits": null }
    })
}

pub fn codex_body() -> Value {
    json!({
        "plan_type": "plus",
        "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": { "used_percent": 60, "limit_window_seconds": 18000, "reset_at": 1791800841 },
            "secondary_window": { "used_percent": 41, "limit_window_seconds": 604800, "reset_at": 1792328613 }
        },
        "credits": { "has_credits": false, "balance": "0" }
    })
}

pub const TEST_TIMEOUT: Duration = Duration::from_millis(300);

/// A Claude login in `dir` holding `token`, pointed at the fake server.
pub fn claude_source(base: &str, dir: &Path, token: &str, expires_at_ms: i64) -> ClaudeSource {
    let file = json!({ "claudeAiOauth": {
        "accessToken": token, "refreshToken": "never-used", "expiresAt": expires_at_ms,
        "subscriptionType": "max", "rateLimitTier": "default_claude_max_20x"
    }});
    write(&dir.join(".credentials.json"), &file);
    ClaudeSource {
        base_url: base.to_string(),
        timeout: TEST_TIMEOUT,
        config_dir: Some(dir.to_path_buf()),
        keychain: false,
    }
}

/// A Codex home in `home` with the given `auth.json`, pointed at the fake server.
pub fn codex_source(base: &str, home: &Path, auth: Option<Value>) -> CodexSource {
    if let Some(auth) = auth {
        write(&home.join("auth.json"), &auth);
    }
    CodexSource {
        base_url: base.to_string(),
        timeout: TEST_TIMEOUT,
        home: Some(home.to_path_buf()),
    }
}

pub fn chatgpt_auth(token: &str) -> Value {
    json!({ "auth_mode": "chatgpt", "OPENAI_API_KEY": null,
            "tokens": { "id_token": "x", "access_token": token, "refresh_token": "never-used", "account_id": "acct-1" },
            "last_refresh": "2026-10-01T00:00:00Z" })
}

/// An unsigned JWT carrying only `exp`.
pub fn jwt(exp: i64) -> String {
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let claims = engine.encode(json!({ "exp": exp }).to_string());
    format!("{}.{claims}.sig", engine.encode("{\"alg\":\"none\"}"))
}

/// A rollout log line as the Codex CLI writes it.
pub fn rollout_line(timestamp: &str, primary_used: f64) -> String {
    json!({ "timestamp": timestamp, "type": "event_msg", "payload": { "type": "token_count",
        "info": null,
        "rate_limits": { "limit_id": "codex",
            "primary": { "used_percent": primary_used, "window_minutes": 300, "resets_at": 1791144682 },
            "secondary": { "used_percent": 8.0, "window_minutes": 10080, "resets_at": 1791659184 },
            "credits": { "has_credits": false, "unlimited": false, "balance": "0" } } } })
    .to_string()
}

pub fn write_rollout(home: &Path, day: &str, name: &str, lines: &[String]) {
    let dir = home.join("sessions").join(day);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), lines.join("\n") + "\n").unwrap();
}

fn write(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, value.to_string()).unwrap();
}
