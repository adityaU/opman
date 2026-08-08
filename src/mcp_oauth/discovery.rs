//! Discovering where to authenticate.
//!
//! Split from [`super::flow`] so that module stays about the browser leg and the token
//! endpoint. A client MUST support both RFC 8414 and OpenID Connect Discovery, so every
//! probe here is a list of candidate URLs rather than a single guess.

use serde::Deserialize;
use url::Url;

use super::OAuthError;

// ── metadata ────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub(super) struct ProtectedResourceMetadata {
    #[serde(default)]
    pub(super) authorization_servers: Vec<String>,
    #[serde(default)]
    pub(super) scopes_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AuthServerMetadata {
    pub(super) issuer: String,
    pub(super) authorization_endpoint: String,
    pub(super) token_endpoint: String,
    #[serde(default)]
    pub(super) registration_endpoint: Option<String>,
    #[serde(default)]
    pub(super) scopes_supported: Vec<String>,
    #[serde(default)]
    pub(super) authorization_response_iss_parameter_supported: bool,
}

/// RFC 9728 §3.1 probe order: the path-inserted form first, then the root.
pub(super) fn prm_urls(resource: &Url) -> Vec<Url> {
    let mut urls = Vec::new();
    let path = resource.path().trim_end_matches('/');
    if !path.is_empty() {
        if let Ok(url) = resource.join(&format!("/.well-known/oauth-protected-resource{path}")) {
            urls.push(url);
        }
    }
    if let Ok(url) = resource.join("/.well-known/oauth-protected-resource") {
        urls.push(url);
    }
    urls
}

/// A client MUST support both discovery families, so this is a list rather than a choice.
pub(super) fn as_urls(issuer: &Url) -> Vec<Url> {
    let path = issuer.path().trim_end_matches('/');
    [
        "/.well-known/oauth-authorization-server",
        "/.well-known/openid-configuration",
    ]
    .iter()
    .flat_map(|suffix| {
        [
            issuer.join(&format!("{suffix}{path}")).ok(),
            issuer.join(suffix).ok(),
        ]
    })
    .flatten()
    .collect()
}

/// Canonical resource identifier: no fragment, no query, no trailing slash.
pub(crate) fn canonical_resource(raw: &str) -> Result<Url, OAuthError> {
    let mut url = Url::parse(raw).map_err(|e| OAuthError::Discovery(e.to_string()))?;
    url.set_fragment(None);
    url.set_query(None);
    let trimmed = url.path().trim_end_matches('/').to_string();
    url.set_path(&trimmed);
    Ok(url)
}

pub(super) async fn fetch_json<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    urls: &[Url],
) -> Option<T> {
    for url in urls {
        let Ok(response) = http.get(url.clone()).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        if let Ok(parsed) = response.json::<T>().await {
            return Some(parsed);
        }
    }
    None
}

pub(super) struct Discovered {
    pub(super) resource: Url,
    pub(super) meta: AuthServerMetadata,
    pub(super) scopes: Vec<String>,
}

/// Probe the server, follow its metadata, and validate the issuer.
///
/// `issuer` is compared against the identifier the request was built from (RFC 8414
/// §3.3), so an unvalidated document can never seed the later `iss` comparison.
pub(super) async fn discover(
    http: &reqwest::Client,
    url: &str,
    extra_scopes: &[String],
) -> Result<Discovered, OAuthError> {
    let resource = canonical_resource(url)?;
    let prm: ProtectedResourceMetadata = fetch_json(http, &prm_urls(&resource))
        .await
        .unwrap_or_default();
    // With no PRM, fall back to treating the server's own origin as the issuer.
    let issuers = if prm.authorization_servers.is_empty() {
        vec![resource.origin().ascii_serialization()]
    } else {
        prm.authorization_servers.clone()
    };
    for candidate in issuers {
        let Ok(issuer_url) = Url::parse(&candidate) else {
            continue;
        };
        let Some(meta) = fetch_json::<AuthServerMetadata>(http, &as_urls(&issuer_url)).await else {
            continue;
        };
        if meta.issuer.trim_end_matches('/') != candidate.trim_end_matches('/') {
            tracing::warn!(expected = %candidate, got = %meta.issuer, "issuer mismatch in AS metadata");
            continue;
        }
        let mut scopes = prm.scopes_supported.clone();
        for scope in extra_scopes {
            if !scopes.contains(scope) {
                scopes.push(scope.clone());
            }
        }
        // Only ask for offline_access when it is advertised; a refresh token is never
        // assumed.
        if meta.scopes_supported.iter().any(|s| s == "offline_access")
            && !scopes.iter().any(|s| s == "offline_access")
        {
            scopes.push("offline_access".to_string());
        }
        return Ok(Discovered {
            resource,
            meta,
            scopes,
        });
    }
    Err(OAuthError::Discovery(
        "no usable authorization server metadata".to_string(),
    ))
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod discovery_tests;
