//! Where a pasted OAuth callback is allowed to go, and the flows waiting for one.
//!
//! Kept apart from the endpoints because this is the security-carrying half: if a caller
//! could influence the address a callback is delivered to, the login endpoint would be a
//! request-forgery hole rather than a convenience for a tunnelled browser.

use std::collections::HashMap;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use url::Url;

use crate::mcp_oauth::OAuthError;

/// The loopback address a pending flow is listening on.
///
/// Held as a whole `Url` so the delivery address is *derived* rather than reassembled
/// from parts, which is what keeps a pasted callback from being able to name a host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Redirect(Url);

impl Redirect {
    /// Read `redirect_uri` back out of the authorize URL the flow just built.
    ///
    /// Taking it from there rather than recomputing it means the address the browser is
    /// sent to and the one a paste is delivered to cannot drift — including the ephemeral
    /// port, which only the flow knows.
    pub(crate) fn from_authorize(authorize: &str) -> Option<Self> {
        let url = Url::parse(authorize).ok()?;
        let raw = url
            .query_pairs()
            .find_map(|(key, value)| (key == "redirect_uri").then(|| value.into_owned()))?;
        let redirect = Url::parse(&raw).ok()?;
        let loopback = matches!(
            redirect.host_str(),
            Some("127.0.0.1" | "localhost" | "[::1]")
        );
        (redirect.scheme() == "http" && loopback && redirect.port().is_some())
            .then_some(Self(redirect))
    }

    /// The address to fetch so the waiting listener receives this response.
    pub(crate) fn delivery(&self, query: &str) -> Url {
        let mut url = self.0.clone();
        url.set_query(Some(query));
        url
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// A login whose browser leg has not landed yet.
pub(crate) struct Pending {
    pub(crate) redirect: Redirect,
    /// Resolves once the whole flow has finished, credential already stored.
    pub(crate) task: JoinHandle<Result<(), OAuthError>>,
}

/// The flows currently waiting on a browser, one per server at most.
///
/// A second login for the same server replaces the first and aborts it, which is also
/// what frees its loopback port — otherwise a fixed `callbackPort` would stay occupied for
/// the full five-minute browser timeout after any abandoned attempt.
#[derive(Default)]
pub struct LoginSessions(Mutex<HashMap<String, Pending>>);

impl LoginSessions {
    pub(crate) async fn arm(&self, name: &str, pending: Pending) {
        if let Some(previous) = self.0.lock().await.insert(name.to_string(), pending) {
            previous.task.abort();
        }
    }

    pub(crate) async fn take(&self, name: &str) -> Option<Pending> {
        self.0.lock().await.remove(name)
    }

    pub(crate) async fn disarm(&self, name: &str) {
        if let Some(pending) = self.take(name).await {
            pending.task.abort();
        }
    }
}

/// Resolve a `${env:NAME}` reference, which is how `mcp.json` spells a secret it does not
/// want to store. Anything else is already the value.
pub(crate) fn resolve_env(raw: &str) -> String {
    let Some(name) = raw
        .strip_prefix("${env:")
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return raw.to_string();
    };
    // An unset variable resolves to empty rather than the unresolved text, so no
    // `${env:…}` string can reach a token endpoint dressed up as a credential.
    std::env::var(name).unwrap_or_default()
}

/// The query of a pasted callback, however much of the URL came with it.
pub(crate) fn callback_query(pasted: &str) -> Option<String> {
    let trimmed = pasted.trim();
    let query = match trimmed.split_once('?') {
        Some((_, query)) => query,
        // A user copying from the address bar sometimes catches only the parameters.
        None => trimmed,
    };
    let query = query.split('#').next().unwrap_or_default();
    (!query.is_empty() && query.contains('=')).then(|| query.to_string())
}

#[cfg(test)]
#[path = "mcp_login_state_tests.rs"]
mod mcp_login_state_tests;
