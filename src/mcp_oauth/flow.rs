//! Discovery, the browser leg, and token exchange.
//!
//! Adapted from the Slack flow in `src/integrations/slack/auth/oauth.rs`: bind the
//! listener *before* opening the browser so there is no race, time out the accept, parse
//! only the request line, and degrade to printing the URL when no browser can be opened.

use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use super::callback::{bind, validate_response, wait_for_callback};
use super::discovery::{discover, AuthServerMetadata};
use super::store::{now_secs, TokenRecord};
use super::{OAuthError, Secret, ServerName};

// ── PKCE ────────────────────────────────────────────────────────────────────────────

struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    /// Derived together, so verifier and challenge cannot drift apart.
    fn generate() -> Self {
        use rand::distributions::Alphanumeric;
        use rand::Rng;
        let verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
        Self {
            verifier,
            challenge,
        }
    }
}

fn nonce(len: usize) -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

// ── registration ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Registered {
    client_id: String,
    #[serde(default)]
    client_secret: Option<String>,
}

async fn client_identity(
    http: &reqwest::Client,
    name: &ServerName,
    meta: &AuthServerMetadata,
    redirect: &str,
    configured_id: &str,
    configured_secret: &str,
) -> Result<(String, Option<Secret>), OAuthError> {
    if !configured_id.is_empty() {
        let secret = (!configured_secret.is_empty()).then(|| Secret::new(configured_secret));
        return Ok((configured_id.to_string(), secret));
    }
    let Some(endpoint) = meta.registration_endpoint.as_deref() else {
        return Err(OAuthError::NeedsPreRegistration(name.to_string()));
    };
    // Register both loopback spellings: servers differ on which they string-match.
    let alt = redirect.replace("127.0.0.1", "localhost");
    let body = serde_json::json!({
        "client_name": format!("opman ({name})"),
        "redirect_uris": [redirect, alt],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let response = http
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| OAuthError::Http(e.to_string()))?;
    if !response.status().is_success() {
        return Err(OAuthError::NeedsPreRegistration(name.to_string()));
    }
    let registered: Registered = response
        .json()
        .await
        .map_err(|e| OAuthError::Http(e.to_string()))?;
    Ok((
        registered.client_id,
        registered.client_secret.map(Secret::new),
    ))
}

// ── token endpoint ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}

/// Complete a login end to end. `on_url` receives the authorization URL, keeping every UI
/// decision — open a browser, print it, hand it to the web UI — out of this module.
#[allow(clippy::too_many_arguments)]
pub async fn login(
    http: &reqwest::Client,
    name: &ServerName,
    url: &str,
    extra_scopes: &[String],
    configured_id: &str,
    configured_secret: &str,
    callback_port: u16,
    on_url: &(dyn Fn(&str) + Send + Sync),
) -> Result<TokenRecord, OAuthError> {
    let discovered = discover(http, url, extra_scopes).await?;
    let (listener, redirect) = bind(callback_port).await?;
    let (client_id, client_secret) = client_identity(
        http,
        name,
        &discovered.meta,
        &redirect,
        configured_id,
        configured_secret,
    )
    .await?;

    let pkce = Pkce::generate();
    let state = nonce(32);
    let scope = discovered.scopes.join(" ");
    let mut authorize = Url::parse(&discovered.meta.authorization_endpoint)
        .map_err(|e| OAuthError::Discovery(e.to_string()))?;
    {
        let mut query = authorize.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &client_id);
        query.append_pair("redirect_uri", &redirect);
        query.append_pair("state", &state);
        query.append_pair("code_challenge", &pkce.challenge);
        query.append_pair("code_challenge_method", "S256");
        // Required on *both* legs, whether or not the server is known to support it.
        query.append_pair("resource", discovered.resource.as_str());
        if !scope.is_empty() {
            query.append_pair("scope", &scope);
        }
    }
    on_url(authorize.as_str());

    let params = wait_for_callback(listener).await?;
    let code = validate_response(
        &params,
        &state,
        &discovered.meta.issuer,
        discovered
            .meta
            .authorization_response_iss_parameter_supported,
    )?;

    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect.clone()),
        ("client_id", client_id.clone()),
        ("code_verifier", pkce.verifier),
        ("resource", discovered.resource.to_string()),
    ];
    if let Some(secret) = &client_secret {
        form.push(("client_secret", secret.expose().to_string()));
    }
    let token: TokenResponse = post_form(http, &discovered.meta.token_endpoint, &form).await?;

    Ok(TokenRecord {
        version: 1,
        resource: discovered.resource.to_string(),
        issuer: discovered.meta.issuer.clone(),
        client_id,
        client_secret,
        access_token: Secret::new(token.access_token),
        refresh_token: token.refresh_token.map(Secret::new),
        token_endpoint: discovered.meta.token_endpoint.clone(),
        expires_at: token.expires_in.map(|secs| now_secs() + secs),
        granted_scopes: split_scopes(token.scope.as_deref()),
        requested_scopes: discovered.scopes,
    })
}

/// Exchange a refresh token. Keeps the previous refresh token when the server does not
/// rotate one, and preserves the requested-scope union across restarts.
pub async fn refresh(
    http: &reqwest::Client,
    record: TokenRecord,
) -> Result<TokenRecord, OAuthError> {
    let Some(refresh_token) = record.refresh_token.clone() else {
        return Err(OAuthError::LoginRequired);
    };
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.expose().to_string()),
        ("client_id", record.client_id.clone()),
        ("resource", record.resource.clone()),
    ];
    if let Some(secret) = &record.client_secret {
        form.push(("client_secret", secret.expose().to_string()));
    }
    let token: TokenResponse = post_form(http, &record.token_endpoint, &form).await?;
    Ok(TokenRecord {
        access_token: Secret::new(token.access_token),
        refresh_token: token
            .refresh_token
            .map(Secret::new)
            .or(record.refresh_token),
        expires_at: token.expires_in.map(|secs| now_secs() + secs),
        granted_scopes: split_scopes(token.scope.as_deref())
            .into_iter()
            .chain(record.granted_scopes.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        ..record
    })
}

async fn post_form<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    endpoint: &str,
    form: &[(&str, String)],
) -> Result<T, OAuthError> {
    let response = http
        .post(endpoint)
        .form(form)
        .send()
        .await
        .map_err(|e| OAuthError::Http(e.to_string()))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        // `invalid_grant` on a refresh means the grant is gone — a retry cannot help, so
        // surface it as "log in again" rather than looping.
        if body.contains("invalid_grant") {
            return Err(OAuthError::LoginRequired);
        }
        return Err(OAuthError::Http(format!("{status}: {body}")));
    }
    serde_json::from_str(&body).map_err(OAuthError::Json)
}

fn split_scopes(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod flow_tests;
