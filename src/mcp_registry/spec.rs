//! The one canonical model of "an MCP server opman wants a runner to run".
//!
//! Built-ins and `mcp.json` entries land here identically, and the four wire renderers
//! see nothing else. Placeholders are parsed into [`Arg`] once at load, so binding a
//! spec to a session is a substitution rather than a re-scan, and [`Presence`] is a
//! declared condition rather than an `if` at each of the four injection sites.

use opman_backend_contracts::RunnerKind;

/// One MCP server, ready to be bound to a session.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerSpec {
    pub(crate) name: Box<str>,
    pub(crate) transport: Transport,
    /// Re-checked at every bind, because what it names can appear after opman starts:
    /// the web server writes `internal.json` mid-run.
    pub(crate) presence: Presence,
    pub(crate) scope: RunnerScope,
    /// Per-server tool-call ceiling, in seconds. Each renderer maps this to its
    /// runner's own key and unit. `None` leaves the runner's default alone.
    ///
    /// Measured defaults differ enough to matter: OpenCode cancels at 60s unless
    /// progress notifications reset its clock, and Codex stops at 300s regardless of
    /// them. Anything that holds a call open — an OAuth wait, most obviously — has to
    /// raise this rather than hope.
    pub(crate) timeout_secs: Option<u32>,
}

/// Where a server's tools actually come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Transport {
    /// A child process the runner launches and talks to over stdio. Every runner
    /// supports this; ACP is the only one that even gates the alternatives.
    Stdio(Stdio),
    /// A network endpoint. Not every runner can dial one — see `bind::RemoteCaps`.
    Remote(Remote),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stdio {
    pub(crate) command: Box<str>,
    pub(crate) args: Box<[Arg]>,
    /// Ordered, so rendering is deterministic and diffable across runners.
    pub(crate) env: Box<[(Box<str>, Arg)]>,
    pub(crate) cwd: Option<Arg>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Remote {
    pub(crate) kind: RemoteKind,
    pub(crate) url: Box<str>,
    pub(crate) headers: Box<[(Box<str>, Arg)]>,
    pub(crate) auth: Auth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteKind {
    Http,
    Sse,
}

/// How the credential for a remote server is obtained.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Auth {
    /// No credential at all.
    #[default]
    None,
    /// `headers` already carry it. Still proxied, so the value never reaches a runner's
    /// argv or environment where `ps` and the agent itself can read it.
    StaticHeader,
    /// Minted per request by `opman mcp-proxy <name>`, with opman owning the token
    /// lifecycle.
    Oauth,
}

impl Remote {
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Headers whose values are literals, for the proxy to attach.
    ///
    /// A `${session}`-dependent header cannot be resolved here — the proxy has no session
    /// — so it is skipped rather than sent half-substituted.
    pub fn literal_headers(&self) -> Vec<(&str, String)> {
        self.headers
            .iter()
            .filter_map(|(name, value)| match value {
                Arg::Lit(text) => Some((name.as_ref(), text.to_string())),
                Arg::Env(var) => std::env::var(var.as_ref()).ok().map(|v| (name.as_ref(), v)),
                _ => None,
            })
            .collect()
    }
}

impl Auth {
    /// Whether opman must front this server locally rather than hand a runner the
    /// endpoint. True for anything carrying a credential — the point of the proxy is
    /// that credentials never leave opman.
    pub(crate) fn needs_proxy(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// A piece of a launch that may depend on the session it is launched for.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Lit(Box<str>),
    /// The session's project directory (`${dir}`).
    Dir,
    /// The opman session id (`${session}`). Unresolvable before an id exists.
    SessionId,
    /// A variable read from opman's own environment (`${env:NAME}`).
    Env(Box<str>),
    /// Literal text with placeholders embedded, e.g. `--profile=${dir}/x`.
    Mixed(Box<[Arg]>),
}

impl Arg {
    /// A plain literal, for the built-ins and for config values with no placeholders.
    pub(crate) fn lit(text: impl Into<Box<str>>) -> Self {
        Self::Lit(text.into())
    }

    /// Whether resolving this depends on a session id existing.
    #[cfg(test)]
    pub(crate) fn needs_session(&self) -> bool {
        match self {
            Self::SessionId => true,
            Self::Mixed(parts) => parts.iter().any(Self::needs_session),
            _ => false,
        }
    }
}

/// A condition re-checked at launch rather than at config load.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Presence {
    #[default]
    Always,
    /// The loopback descriptor `~/.config/opman/internal.json` exists, i.e. the web
    /// server is up. Bind-time so a runner picks the server up mid-run.
    KanbanDescriptor,
    /// The named variable is set in opman's own environment.
    Env(Box<str>),
}

impl Presence {
    pub(crate) fn met(&self) -> bool {
        match self {
            Self::Always => true,
            Self::KanbanDescriptor => dirs::config_dir()
                .map(|dir| dir.join("opman").join("internal.json").is_file())
                .unwrap_or(false),
            Self::Env(name) => std::env::var_os(name.as_ref()).is_some(),
        }
    }
}

/// Which runner slots a server is offered to. An empty allow-list means every runner;
/// the deny-list is applied after it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunnerScope {
    only: Box<[RunnerKind]>,
    except: Box<[RunnerKind]>,
}

impl RunnerScope {
    pub(crate) fn new(only: Vec<RunnerKind>, except: Vec<RunnerKind>) -> Self {
        Self {
            only: only.into_boxed_slice(),
            except: except.into_boxed_slice(),
        }
    }

    pub(crate) fn admits(&self, runner: &RunnerKind) -> bool {
        if self.except.contains(runner) {
            return false;
        }
        self.only.is_empty() || self.only.contains(runner)
    }
}

impl ServerSpec {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn transport(&self) -> &Transport {
        &self.transport
    }

    pub(crate) fn timeout_secs(&self) -> Option<u32> {
        self.timeout_secs
    }

    pub(crate) fn admits(&self, runner: &RunnerKind) -> bool {
        self.scope.admits(runner)
    }

    /// True when any part of this spec resolves differently once a session id exists.
    ///
    /// Every runner opman drives now learns its session id before it is handed a server
    /// list, so nothing re-sends on this at runtime. It stays as the one place that
    /// knows where a session placeholder may hide, and the builtin specs are asserted
    /// against it — "agent-manager routes by session" is a property of the definition,
    /// not of any one renderer.
    #[cfg(test)]
    pub(crate) fn binds_session(&self) -> bool {
        match &self.transport {
            Transport::Stdio(stdio) => {
                stdio.args.iter().any(Arg::needs_session)
                    || stdio.env.iter().any(|(_, value)| value.needs_session())
                    || stdio.cwd.as_ref().is_some_and(Arg::needs_session)
            }
            Transport::Remote(remote) => remote
                .headers
                .iter()
                .any(|(_, value)| value.needs_session()),
        }
    }

    /// A stdio server with literal arguments — the shape every built-in has.
    pub(crate) fn stdio(
        name: impl Into<Box<str>>,
        command: impl Into<Box<str>>,
        args: Vec<Arg>,
        env: Vec<(Box<str>, Arg)>,
    ) -> Self {
        Self {
            name: name.into(),
            transport: Transport::Stdio(Stdio {
                command: command.into(),
                args: args.into_boxed_slice(),
                env: env.into_boxed_slice(),
                cwd: None,
            }),
            presence: Presence::Always,
            scope: RunnerScope::default(),
            timeout_secs: None,
        }
    }

    pub(crate) fn with_presence(mut self, presence: Presence) -> Self {
        self.presence = presence;
        self
    }
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod spec_tests;
