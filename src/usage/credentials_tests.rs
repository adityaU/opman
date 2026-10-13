use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::usage::fake_provider::jwt;

/// Serialises the tests that change process-wide env vars.
static ENV: Mutex<()> = Mutex::new(());

#[test]
fn parses_a_claude_login() {
    let text = json!({ "claudeAiOauth": {
        "accessToken": "tok", "expiresAt": 42, "subscriptionType": "pro", "rateLimitTier": "x"
    }})
    .to_string();
    let Login::Found(login) = parse_claude(&text) else {
        panic!("expected a login");
    };
    assert_eq!(login.access_token.expose(), "tok");
    assert_eq!(login.expires_at_ms, Some(42));
    assert_eq!(login.plan.as_deref(), Some("pro"));
    // A token never shows up in debug output.
    assert!(!format!("{login:?}").contains("tok\""));
}

#[test]
fn claude_file_without_oauth_is_missing() {
    assert!(matches!(parse_claude("{}"), Login::Missing));
    assert!(matches!(parse_claude("{not json"), Login::Unreadable(_)));
}

#[test]
fn parses_codex_logins() {
    let chatgpt =
        json!({ "auth_mode": "chatgpt", "tokens": { "access_token": jwt(99), "account_id": "a" } });
    let Login::Found(CodexLogin::Chatgpt {
        account_id,
        expires_at,
        ..
    }) = parse_codex(&chatgpt.to_string())
    else {
        panic!("expected a ChatGPT login");
    };
    assert_eq!(account_id.as_deref(), Some("a"));
    assert_eq!(expires_at, Some(99));

    let key = json!({ "OPENAI_API_KEY": "sk-1", "tokens": null }).to_string();
    assert!(matches!(
        parse_codex(&key),
        Login::Found(CodexLogin::ApiKey)
    ));
    let key_mode = json!({ "auth_mode": "apikey", "tokens": { "access_token": "t" } }).to_string();
    assert!(matches!(
        parse_codex(&key_mode),
        Login::Found(CodexLogin::ApiKey)
    ));
    assert!(matches!(parse_codex("{}"), Login::Missing));
}

#[test]
fn jwt_expiry_tolerates_garbage() {
    assert_eq!(jwt_expiry(&jwt(1234)), Some(1234));
    assert_eq!(jwt_expiry("opaque-token"), None);
    assert_eq!(jwt_expiry("a.!!!.c"), None);
}

#[tokio::test]
async fn reads_login_files_from_their_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let claude = json!({ "claudeAiOauth": { "accessToken": "t" } }).to_string();
    std::fs::write(dir.path().join(".credentials.json"), claude).unwrap();
    assert!(matches!(
        claude_login(Some(dir.path()), false).await,
        Login::Found(_)
    ));
    assert!(matches!(
        codex_login(Some(dir.path())).await,
        Login::Missing
    ));
    assert!(matches!(claude_login(None, false).await, Login::Missing));
}

#[test]
fn env_vars_override_the_default_dirs() {
    let _guard = ENV.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let saved = (
        std::env::var_os("CLAUDE_CONFIG_DIR"),
        std::env::var_os("CODEX_HOME"),
    );
    std::env::set_var("CLAUDE_CONFIG_DIR", "/tmp/usage-claude");
    std::env::set_var("CODEX_HOME", "/tmp/usage-codex");
    assert_eq!(claude_dir(), Some(PathBuf::from("/tmp/usage-claude")));
    assert_eq!(codex_home(), Some(PathBuf::from("/tmp/usage-codex")));
    assert!(!use_keychain());
    std::env::set_var("CLAUDE_CONFIG_DIR", "");
    std::env::remove_var("CODEX_HOME");
    let home = dirs::home_dir().unwrap();
    assert_eq!(claude_dir(), Some(home.join(".claude")));
    assert_eq!(codex_home(), Some(home.join(".codex")));
    for (var, value) in [("CLAUDE_CONFIG_DIR", saved.0), ("CODEX_HOME", saved.1)] {
        match value {
            Some(value) => std::env::set_var(var, value),
            None => std::env::remove_var(var),
        }
    }
}
