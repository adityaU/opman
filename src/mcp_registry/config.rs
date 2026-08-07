//! `~/.config/opman/mcp.json` — the user's external MCP servers.
//!
//! An entry naming one of opman's built-ins *patches* it when it declares no transport
//! of its own — so `{"time": {"enabled": false}}` turns a built-in off, and
//! `{"terminal": {"timeoutSecs": 120}}` retimes one, without the user restating a
//! command they never wrote. An entry that does declare a `command` or `url` replaces the
//! built-in outright. A missing or malformed file is never fatal: opman still starts with
//! its own servers.

use std::collections::BTreeMap;
use std::path::PathBuf;

use opman_backend_contracts::RunnerKind;
use serde::{Deserialize, Serialize};

use super::parse;
use super::spec::{
    Auth, Presence, Remote, RemoteKind, RunnerScope, ServerSpec, Stdio, Transport,
};

/// The whole `mcp.json` document.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct McpConfig {
    pub servers: BTreeMap<String, ServerConfig>,
}

/// One declared server. Every field is optional; which ones are set decides the shape.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerConfig {
    /// `stdio` | `http` | `sse`. Inferred from `command` vs `url` when empty.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cwd: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// `none` | `static` | `oauth`. Anything but `none` is fronted by the local proxy so
    /// the credential never reaches a runner.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub auth: String,
    /// Only offer this server when the named variable is set.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub requires_env: String,
    /// Runner slots to offer it to. Empty means all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runners: Vec<RunnerKind>,
    /// Runner slots to withhold it from, applied after `runners`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_runners: Vec<RunnerKind>,
    /// Per-server tool-call ceiling in seconds, mapped to each runner's own key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u32>,
    pub enabled: bool,

    // -- OAuth identity, read by `mcp_oauth`. Kept here so one file describes a server.
    /// Pre-registered client id, for authorization servers without dynamic registration.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub client_id: String,
    /// Pre-registered client secret, or `${env:VAR}` naming one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub client_secret: String,
    /// HTTPS URL used directly as the client id (Client ID Metadata Document).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub client_id_metadata_url: String,
    /// Fixed loopback callback port. Required when `client_id` is set, because a
    /// pre-registered redirect URI cannot use an ephemeral port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_port: Option<u16>,
    /// Scopes always requested on top of whatever discovery advertises.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            r#type: String::new(),
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: String::new(),
            url: String::new(),
            headers: BTreeMap::new(),
            auth: String::new(),
            requires_env: String::new(),
            runners: Vec::new(),
            exclude_runners: Vec::new(),
            timeout_secs: None,
            enabled: true,
            client_id: String::new(),
            client_secret: String::new(),
            client_id_metadata_url: String::new(),
            callback_port: None,
            scopes: Vec::new(),
        }
    }
}

impl ServerConfig {
    /// Whether this entry describes how to launch or reach a server at all.
    ///
    /// An entry that does not is a *patch* on a built-in of the same name — turning it
    /// off, retiming it, or narrowing which runners see it — rather than a redefinition.
    /// That distinction is what lets `{"time": {"enabled": false}}` work without the user
    /// having to restate a command they never wrote.
    pub(crate) fn defines_transport(&self) -> bool {
        !self.command.is_empty() || !self.url.is_empty()
    }

    /// Apply the patchable fields of this entry to a built-in spec.
    ///
    /// Returns `None` when the entry disables the built-in. Only fields that make sense
    /// independently of a transport are applied — the rest would be meaningless without
    /// one, and are why [`Self::defines_transport`] exists.
    pub(crate) fn patch(&self, mut spec: ServerSpec) -> Option<ServerSpec> {
        if !self.enabled {
            return None;
        }
        if self.timeout_secs.is_some() {
            spec.timeout_secs = self.timeout_secs;
        }
        if !self.runners.is_empty() || !self.exclude_runners.is_empty() {
            spec.scope = RunnerScope::new(self.runners.clone(), self.exclude_runners.clone());
        }
        if !self.requires_env.is_empty() {
            spec.presence = self.presence();
        }
        Some(spec)
    }

    /// Whether this entry describes a remote endpoint rather than a child process.
    pub(crate) fn remote_kind(&self) -> Option<RemoteKind> {
        match self.r#type.as_str() {
            "http" | "streamable-http" => Some(RemoteKind::Http),
            "sse" => Some(RemoteKind::Sse),
            "stdio" | "local" => None,
            // No explicit type: a url means remote, anything else means stdio.
            _ if !self.url.is_empty() => Some(RemoteKind::Http),
            _ => None,
        }
    }

    fn auth_mode(&self) -> Auth {
        match self.auth.as_str() {
            "oauth" => Auth::Oauth,
            "static" | "staticHeader" | "header" => Auth::StaticHeader,
            // Headers on a server with no declared auth still carry a credential often
            // enough that treating them as public would be the wrong default.
            _ if !self.headers.is_empty() => Auth::StaticHeader,
            _ => Auth::None,
        }
    }

    fn presence(&self) -> Presence {
        if self.requires_env.is_empty() {
            return Presence::Always;
        }
        Presence::Env(self.requires_env.as_str().into())
    }

    /// Build the canonical spec, or `None` when the entry is disabled or unusable.
    pub(crate) fn to_spec(&self, name: &str) -> Option<ServerSpec> {
        if !self.enabled {
            return None;
        }
        let transport = match self.remote_kind() {
            Some(kind) => {
                if self.url.is_empty() {
                    tracing::warn!(server = name, "mcp.json entry has no url; ignoring");
                    return None;
                }
                Transport::Remote(Remote {
                    kind,
                    url: self.url.as_str().into(),
                    headers: parse::pairs(self.headers.iter(), name).into_boxed_slice(),
                    auth: self.auth_mode(),
                })
            }
            None => {
                if self.command.is_empty() {
                    tracing::warn!(server = name, "mcp.json entry has no command; ignoring");
                    return None;
                }
                Transport::Stdio(Stdio {
                    command: self.command.as_str().into(),
                    args: self
                        .args
                        .iter()
                        .map(|value| parse::arg(value, name))
                        .collect(),
                    env: parse::pairs(self.env.iter(), name).into_boxed_slice(),
                    cwd: (!self.cwd.is_empty()).then(|| parse::arg(&self.cwd, name)),
                })
            }
        };
        // A proxied server may hold a call open while the user completes a browser
        // login, so it needs a ceiling well above every runner's default. Measured:
        // OpenCode cancels at 60s without progress notifications, Codex at 300s even
        // with them.
        let timeout_secs = self.timeout_secs.or_else(|| {
            matches!(&transport, Transport::Remote(remote) if remote.auth.needs_proxy())
                .then_some(super::PROXY_TIMEOUT_SECS)
        });
        Some(ServerSpec {
            name: name.into(),
            transport,
            presence: self.presence(),
            scope: RunnerScope::new(self.runners.clone(), self.exclude_runners.clone()),
            timeout_secs,
        })
    }
}

/// Path of the user config file. `$OPMAN_MCP_CONFIG` wins when set.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("OPMAN_MCP_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    dirs::config_dir().map(|dir| dir.join("opman").join("mcp.json"))
}

/// Read the user's file. A missing file is an empty config; a malformed one warns and is
/// ignored, so a typo cannot stop opman from starting.
pub fn load() -> McpConfig {
    let Some(path) = config_path() else {
        return McpConfig::default();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return McpConfig::default();
    };
    match serde_json::from_str(&raw) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(path = %path.display(), "ignoring malformed mcp.json: {error}");
            McpConfig::default()
        }
    }
}

/// Ensure a placeholder-free literal survives a round trip, for callers writing the file
/// back from the management API.
pub fn save(config: &McpConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
