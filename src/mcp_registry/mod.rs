//! Every MCP server opman wants runners to run: its own built-ins plus the user's
//! `~/.config/opman/mcp.json`, resolved once and rendered into each runner's native wire
//! shape.
//!
//! Before this module the same server list was hand-rolled four times, once per runner,
//! with the gating conditions duplicated alongside it. The pipeline now narrows in
//! stages — [`config::ServerConfig`] (raw, merged) into [`spec::ServerSpec`] (canonical,
//! placeholders parsed) into [`bind::Wire`] (bound to a session, runner-legal) — and only
//! the last of those reaches a renderer.

pub mod bind;
pub mod builtin;
pub mod config;
pub mod handle;
mod parse;
pub mod render;
pub mod spec;

use std::path::PathBuf;
use std::sync::Arc;

use opman_backend_contracts::RunnerKind;

pub use bind::{Bind, RemoteCaps};
pub use handle::RegistryHandle;
pub use spec::ServerSpec;

/// Ceiling for a server fronted by `opman mcp-proxy`, in seconds.
///
/// The proxy holds a tool call open while the user authenticates in a browser, emitting
/// progress notifications to keep the runner's clock alive. This has to exceed that wait
/// with margin: the wait itself is [`AUTH_WAIT_SECS`].
pub const PROXY_TIMEOUT_SECS: u32 = 900;

/// How long the proxy will hold a call open waiting for a credential to appear.
pub const AUTH_WAIT_SECS: u64 = 600;

/// The resolved server set, shared by every engine.
#[derive(Debug, Default)]
pub struct McpRegistry {
    /// opman's own executable, resolved once rather than at each of the four old
    /// injection sites. Every built-in launches it, and so does every proxy.
    exe: Box<str>,
    servers: Box<[ServerSpec]>,
}

impl McpRegistry {
    /// Built-ins with the user's `mcp.json` merged over them.
    ///
    /// Must run after opman has set `OPMAN_AGENT_MANAGER_SOCKET` and after ACP agents are
    /// registered, so presence checks and `RunnerKind` scoping both resolve.
    pub fn load() -> Self {
        Self::from_config(config::load())
    }

    /// Registry used by `mcp-code`. It keeps user-defined servers from the normal
    /// configuration and adds every built-in so code mode can manage and enable them.
    pub fn load_for_code_mode() -> Self {
        Self::load().with_all_builtins()
    }

    /// Just the built-ins, ignoring any user config. For tests and for callers that
    /// need a registry before the config directory is meaningful.
    pub fn builtins() -> Self {
        Self::from_config(config::McpConfig::default())
    }

    pub(crate) fn from_config(user: config::McpConfig) -> Self {
        let exe = current_exe();
        let mut builtins = builtin::servers(&exe);
        let mut specs: Vec<ServerSpec> = Vec::new();

        // A user entry that names a built-in either patches it (no transport of its own)
        // or replaces it outright. Anything else is a new server.
        for (name, entry) in &user.servers {
            match builtins
                .iter()
                .position(|spec| spec.name() == name.as_str())
            {
                Some(index) if !entry.defines_transport() => {
                    specs.extend(entry.patch(builtins.remove(index)));
                }
                Some(index) => {
                    builtins.remove(index);
                    specs.extend(entry.to_spec(name));
                }
                None => specs.extend(entry.to_spec(name)),
            }
        }
        // What is left has no entry of its own: the ones that need switching on stay out.
        builtins.retain(|spec| !builtin::is_off_until_enabled(spec.name()));
        specs.append(&mut builtins);
        specs.sort_by(|a, b| a.name().cmp(b.name()));
        Self {
            exe: exe.into(),
            servers: specs.into_boxed_slice(),
        }
    }

    /// Test constructor: an exact server set, no filesystem, no environment.
    #[cfg(test)]
    pub(crate) fn from_specs(servers: Vec<ServerSpec>) -> Self {
        Self {
            exe: "/opman".into(),
            servers: servers.into_boxed_slice(),
        }
    }

    /// Bind context for one session, against this registry's resolved executable.
    pub fn bind<'a>(&'a self, dir: &'a str, session: Option<&'a str>) -> Bind<'a> {
        Bind::new(&self.exe, dir, session)
    }

    /// The servers offered to one runner slot. Borrowing: engines hold an
    /// `Arc<McpRegistry>` and filter per call rather than each owning a copy.
    pub fn for_runner<'a>(
        &'a self,
        runner: &'a RunnerKind,
    ) -> impl Iterator<Item = &'a ServerSpec> + 'a {
        self.servers.iter().filter(move |spec| spec.admits(runner))
    }

    /// Whether a server of this name is on offer to anyone.
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// Every server, for the management API's listing.
    pub fn all(&self) -> impl Iterator<Item = &ServerSpec> {
        self.servers.iter()
    }

    fn with_all_builtins(mut self) -> Self {
        let existing = std::mem::take(&mut self.servers);
        let exe = self.exe.as_ref();
        let mut merged = builtin::servers(exe);
        for spec in existing {
            if !merged.iter().any(|builtin| builtin.name() == spec.name()) {
                merged.push(spec);
            }
        }
        self.servers = merged.into_boxed_slice();
        self.servers
            .sort_by(|left, right| left.name().cmp(right.name()));
        self
    }

    /// Look one server up by name, for the proxy and the management API.
    pub fn get(&self, name: &str) -> Option<&ServerSpec> {
        self.servers.iter().find(|spec| spec.name() == name)
    }

    /// One server as a child process opman can talk to itself.
    ///
    /// Stdio-only on purpose. A remote or credential-bearing server comes back as
    /// `opman mcp-proxy <name>`, so the tool probe reads the very server a runner is
    /// handed — same auth, same degraded listing when a login is missing — and the
    /// credential still never leaves opman.
    pub fn stdio_launch<'a>(&'a self, name: &str, at: Bind<'a>) -> Option<bind::WireStdio<'a>> {
        self.get(name)?.stdio_launch(at)
    }
}

/// opman's own executable, resolved once instead of at each of the four old sites.
fn current_exe() -> String {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("opman"))
        .to_string_lossy()
        .into_owned()
}

/// A registry shared across engines. Swappable, so `mcp.json` edits apply without a
/// restart — see [`RegistryHandle`].
pub type SharedRegistry = RegistryHandle;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
