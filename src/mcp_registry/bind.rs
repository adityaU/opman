//! Binding a [`ServerSpec`] to one session, for one runner's capabilities.
//!
//! This is where "a remote server cannot be rendered into a shape that does not support
//! it" is enforced. [`Wire`] has no public constructor: [`ServerSpec::bind`] is the sole
//! producer, and it rewrites every credential-bearing or undialable remote into the
//! local proxy's stdio command. A renderer therefore never has to ask whether its runner
//! can dial an endpoint, and cannot forget to.

use std::borrow::Cow;

use super::spec::{Arg, Remote, RemoteKind, ServerSpec, Stdio, Transport};

/// Everything a spec needs to become a concrete launch.
#[derive(Clone, Copy, Debug)]
pub struct Bind<'a> {
    exe: &'a str,
    dir: &'a str,
    /// `None` before an id exists: OpenCode's config is process-wide, and Codex's
    /// `thread/start` happens before the thread has an id.
    session: Option<&'a str>,
}

impl<'a> Bind<'a> {
    pub fn new(exe: &'a str, dir: &'a str, session: Option<&'a str>) -> Self {
        Self { exe, dir, session }
    }
}

/// Which remote transports the target runner can dial for itself. Anything it cannot
/// dial is served by `opman mcp-proxy` instead, so no server is ever silently dropped
/// for lack of a transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteCaps {
    http: bool,
    sse: bool,
}

impl RemoteCaps {
    /// Claude Code: documented `type: "http" | "sse" | "ws"`.
    pub const CLAUDE: Self = Self {
        http: true,
        sse: true,
    };
    /// OpenCode's `type: "remote"` and Codex's `mcp_servers.<n>.url` are both a single
    /// streamable-HTTP form with no separate SSE flavour.
    pub const HTTP_ONLY: Self = Self {
        http: true,
        sse: false,
    };
    /// No remote transport at all — every remote server goes through the proxy. The
    /// default for an ACP agent that did not advertise `mcpCapabilities`.
    pub const STDIO_ONLY: Self = Self {
        http: false,
        sse: false,
    };

    pub const fn new(http: bool, sse: bool) -> Self {
        Self { http, sse }
    }

    fn dials(self, kind: RemoteKind) -> bool {
        match kind {
            RemoteKind::Http => self.http,
            RemoteKind::Sse => self.sse,
        }
    }
}

/// A server bound to one session, in a form the target runner is *known* to accept.
#[derive(Clone, Debug, PartialEq)]
pub enum Wire<'a> {
    Stdio(WireStdio<'a>),
    Remote(WireRemote<'a>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WireStdio<'a> {
    pub command: &'a str,
    pub args: Vec<Cow<'a, str>>,
    pub env: Vec<(&'a str, Cow<'a, str>)>,
    pub cwd: Option<Cow<'a, str>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WireRemote<'a> {
    pub kind: RemoteKind,
    pub url: &'a str,
    pub headers: Vec<(&'a str, Cow<'a, str>)>,
}

impl ServerSpec {
    /// Bind this spec for one session against one runner's capabilities.
    ///
    /// `None` means "do not offer this server here": its presence condition is unmet, or
    /// a positional argument referenced something that does not exist yet — a
    /// `${session}` under OpenCode's process-wide config, say. An unresolvable
    /// *environment or header value* drops only that pair, which is how the
    /// agent-manager bridge already behaves on Codex's `thread/start`.
    pub(crate) fn bind<'a>(&'a self, at: Bind<'a>, caps: RemoteCaps) -> Option<Wire<'a>> {
        if !self.presence.met() {
            return None;
        }
        match &self.transport {
            Transport::Stdio(stdio) => stdio.bind(at).map(Wire::Stdio),
            // The whole fan-out asymmetry, decided once: a credential-bearing server is
            // proxied however capable the runner is, and an undialable transport is
            // proxied rather than dropped.
            Transport::Remote(remote) => {
                if remote.auth.needs_proxy() || !caps.dials(remote.kind) {
                    return Some(Wire::Stdio(self.proxy_launch(at)));
                }
                remote.bind(at).map(Wire::Remote)
            }
        }
    }

    /// This spec as a child process, whatever its transport.
    ///
    /// The stdio-only end of [`Self::bind`], returned unwrapped: a caller that speaks to
    /// the server itself — the settings page's tool probe — has no remote client and
    /// would only have to re-handle a `Wire::Remote` that cannot occur here.
    pub(crate) fn stdio_launch<'a>(&'a self, at: Bind<'a>) -> Option<WireStdio<'a>> {
        if !self.presence.met() {
            return None;
        }
        match &self.transport {
            Transport::Stdio(stdio) => stdio.bind(at),
            Transport::Remote(_) => Some(self.proxy_launch(at)),
        }
    }

    /// The stdio command that fronts this server locally. `opman mcp-proxy <name>`
    /// re-reads the registry, dials the endpoint, and injects a fresh credential per
    /// request — so the remote's URL, headers, and token lifetime never reach a runner.
    fn proxy_launch<'a>(&'a self, at: Bind<'a>) -> WireStdio<'a> {
        WireStdio {
            command: at.exe,
            args: vec![Cow::Borrowed("mcp-proxy"), Cow::Borrowed(&self.name)],
            env: Vec::new(),
            cwd: None,
        }
    }
}

impl Stdio {
    fn bind<'a>(&'a self, at: Bind<'a>) -> Option<WireStdio<'a>> {
        let mut args = Vec::with_capacity(self.args.len());
        for arg in self.args.iter() {
            // A positional hole cannot be skipped without changing what the command
            // means, so an unresolvable one takes the whole server with it.
            args.push(resolve(arg, at)?);
        }
        let cwd = match &self.cwd {
            Some(arg) => Some(resolve(arg, at)?),
            None => None,
        };
        Some(WireStdio {
            command: &self.command,
            args,
            env: resolve_pairs(&self.env, at),
            cwd,
        })
    }
}

impl Remote {
    fn bind<'a>(&'a self, at: Bind<'a>) -> Option<WireRemote<'a>> {
        Some(WireRemote {
            kind: self.kind,
            url: &self.url,
            headers: resolve_pairs(&self.headers, at),
        })
    }
}

/// Name/value pairs where an unresolvable value drops just that pair.
fn resolve_pairs<'a>(
    pairs: &'a [(Box<str>, Arg)],
    at: Bind<'a>,
) -> Vec<(&'a str, Cow<'a, str>)> {
    pairs
        .iter()
        .filter_map(|(name, value)| Some((name.as_ref(), resolve(value, at)?)))
        .collect()
}

fn resolve<'a>(arg: &'a Arg, at: Bind<'a>) -> Option<Cow<'a, str>> {
    match arg {
        Arg::Lit(text) => Some(Cow::Borrowed(text.as_ref())),
        Arg::Dir => Some(Cow::Borrowed(at.dir)),
        Arg::SessionId => at.session.map(Cow::Borrowed),
        Arg::Env(name) => std::env::var(name.as_ref()).ok().map(Cow::Owned),
        Arg::Mixed(parts) => {
            let mut out = String::new();
            for part in parts.iter() {
                out.push_str(&resolve(part, at)?);
            }
            Some(Cow::Owned(out))
        }
    }
}

#[cfg(test)]
#[path = "bind_tests.rs"]
mod bind_tests;
