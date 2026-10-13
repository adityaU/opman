//! Reading the vendors' CLI logins, read-only. Tokens are wrapped in [`Secret`] so a
//! stray `{:?}` cannot print one, and nothing here ever writes a credential back.

use std::fmt;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;

/// A bearer token. Only [`Secret::expose`] yields the value, for the one header that
/// sends it to its own provider.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// What looking for a login found.
#[derive(Debug)]
pub enum Login<T> {
    Missing,
    Unreadable(String),
    Found(T),
}

#[derive(Debug)]
pub struct ClaudeLogin {
    pub access_token: Secret,
    pub expires_at_ms: Option<i64>,
    pub plan: Option<String>,
}

#[derive(Debug)]
pub enum CodexLogin {
    ApiKey,
    Chatgpt {
        access_token: Secret,
        account_id: Option<String>,
        /// The access token's own `exp` claim, in unix seconds.
        expires_at: Option<i64>,
    },
}

#[derive(Deserialize)]
struct ClaudeFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<ClaudeOauth>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeOauth {
    access_token: Option<String>,
    expires_at: Option<i64>,
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct CodexFile {
    auth_mode: Option<String>,
    #[serde(rename = "OPENAI_API_KEY")]
    api_key: Option<String>,
    tokens: Option<CodexTokens>,
}

#[derive(Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

/// `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn claude_dir() -> Option<PathBuf> {
    env_dir("CLAUDE_CONFIG_DIR").or_else(|| Some(dirs::home_dir()?.join(".claude")))
}

/// `$CODEX_HOME`, else `~/.codex`.
pub fn codex_home() -> Option<PathBuf> {
    env_dir("CODEX_HOME").or_else(|| Some(dirs::home_dir()?.join(".codex")))
}

fn env_dir(var: &str) -> Option<PathBuf> {
    let value = std::env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

async fn read_text(path: &Path) -> Login<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => Login::Found(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Login::Missing,
        Err(error) => Login::Unreadable(format!("Could not read {}: {error}", path.display())),
    }
}

/// The Claude Code login. On macOS it lives in the login Keychain (unless a custom
/// config dir is in use); the credentials file is the fallback everywhere.
pub async fn claude_login(dir: Option<&Path>, keychain: bool) -> Login<ClaudeLogin> {
    if keychain {
        if let Some(text) = keychain_credentials().await {
            return parse_claude(&text);
        }
    }
    let Some(dir) = dir else {
        return Login::Missing;
    };
    match read_text(&dir.join(".credentials.json")).await {
        Login::Found(text) => parse_claude(&text),
        Login::Missing => Login::Missing,
        Login::Unreadable(why) => Login::Unreadable(why),
    }
}

pub fn parse_claude(text: &str) -> Login<ClaudeLogin> {
    let Ok(file) = serde_json::from_str::<ClaudeFile>(text) else {
        return Login::Unreadable("Claude credentials are not valid JSON".into());
    };
    let Some(oauth) = file.oauth else {
        return Login::Missing;
    };
    let Some(token) = oauth.access_token.filter(|token| !token.is_empty()) else {
        return Login::Missing;
    };
    Login::Found(ClaudeLogin {
        access_token: Secret(token),
        expires_at_ms: oauth.expires_at,
        plan: oauth.subscription_type.filter(|plan| !plan.is_empty()),
    })
}

#[cfg(target_os = "macos")]
async fn keychain_credentials() -> Option<String> {
    let output = tokio::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_string())
}

#[cfg(not(target_os = "macos"))]
async fn keychain_credentials() -> Option<String> {
    None
}

/// Whether to look in the Keychain: only on macOS, and only for the default config dir,
/// because Claude Code keys a custom dir's entry under a different service name.
pub fn use_keychain() -> bool {
    cfg!(target_os = "macos") && env_dir("CLAUDE_CONFIG_DIR").is_none()
}

pub async fn codex_login(home: Option<&Path>) -> Login<CodexLogin> {
    let Some(home) = home else {
        return Login::Missing;
    };
    match read_text(&home.join("auth.json")).await {
        Login::Found(text) => parse_codex(&text),
        Login::Missing => Login::Missing,
        Login::Unreadable(why) => Login::Unreadable(why),
    }
}

pub fn parse_codex(text: &str) -> Login<CodexLogin> {
    let Ok(file) = serde_json::from_str::<CodexFile>(text) else {
        return Login::Unreadable("Codex auth.json is not valid JSON".into());
    };
    let token = file
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.access_token.clone())
        .filter(|token| !token.is_empty());
    let api_key_mode = file
        .auth_mode
        .as_deref()
        .is_some_and(|mode| mode.eq_ignore_ascii_case("apikey") || mode == "api_key");
    let has_api_key = file.api_key.as_deref().is_some_and(|key| !key.is_empty());
    let Some(token) = token.filter(|_| !api_key_mode) else {
        if api_key_mode || has_api_key {
            return Login::Found(CodexLogin::ApiKey);
        }
        return Login::Missing;
    };
    let expires_at = jwt_expiry(&token);
    Login::Found(CodexLogin::Chatgpt {
        account_id: file.tokens.and_then(|tokens| tokens.account_id),
        access_token: Secret(token),
        expires_at,
    })
}

/// The `exp` claim of a JWT, without verifying it: this only decides whether to bother
/// the provider, which verifies the token itself.
pub fn jwt_expiry(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("exp")?.as_i64()
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
