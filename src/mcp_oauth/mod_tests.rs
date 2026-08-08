//! The non-interactive surface the proxy depends on.

use super::*;

fn temp_store(tag: &str) -> (TokenStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("opman-oauth-mod-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create");
    (TokenStore::with_dir(dir.clone()), dir)
}

fn name() -> ServerName {
    ServerName::parse("linear").expect("valid")
}

/// A proxy child must never open a browser: a background process ambushing the user with
/// a window is worse than an actionable error the model can relay.
#[tokio::test]
async fn access_token_reports_login_required_rather_than_prompting() {
    let (store, dir) = temp_store("nologin");
    let http = reqwest::Client::new();
    let error = access_token(&http, &store, &name())
        .await
        .expect_err("no token");
    assert!(matches!(error, OAuthError::LoginRequired));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn is_authenticated_follows_the_stored_credential() {
    let (store, dir) = temp_store("status");
    assert!(!is_authenticated(&store, &name()));
    let record = TokenRecord {
        version: 1,
        resource: "https://x/mcp".into(),
        issuer: "https://as".into(),
        client_id: "c".into(),
        client_secret: None,
        access_token: Secret::new("at"),
        refresh_token: None,
        token_endpoint: "https://as/token".into(),
        expires_at: None,
        granted_scopes: Vec::new(),
        requested_scopes: Vec::new(),
    };
    store.save(&name(), &record).expect("save");
    assert!(is_authenticated(&store, &name()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn logout_clears_the_credential() {
    let (store, dir) = temp_store("logout");
    let record = TokenRecord {
        version: 1,
        resource: "https://x/mcp".into(),
        issuer: "https://as".into(),
        client_id: "c".into(),
        client_secret: None,
        access_token: Secret::new("at"),
        refresh_token: None,
        token_endpoint: "https://as/token".into(),
        expires_at: None,
        granted_scopes: Vec::new(),
        requested_scopes: Vec::new(),
    };
    store.save(&name(), &record).expect("save");
    logout(&store, &name()).expect("logout");
    assert!(!is_authenticated(&store, &name()));
    let _ = std::fs::remove_dir_all(dir);
}
