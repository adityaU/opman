//! opman's own MCP servers, expressed as ordinary [`ServerSpec`]s.
//!
//! Seeding them into the same map the user's `mcp.json` merges over is what lets a
//! one-line override disable or retune a built-in, and it collapses the gating that used
//! to be duplicated across four injection sites — three separate `internal.json`
//! existence checks and three separate socket-env checks — into one declaration each.

use super::spec::{Arg, Presence, ServerSpec};
use super::PROXY_TIMEOUT_SECS;

/// The environment variable the terminal, neovim, and agent-manager bridges read to
/// route back to the session that launched them.
const SESSION_ENV: &str = "OPENCODE_SESSION_ID";
/// Set by opman before any runner is spawned; its presence is what says the in-process
/// agent-manager listener exists to talk to.
const MANAGER_SOCKET: &str = crate::mcp_agent_manager::SOCKET_ENV;

/// Which of opman's own servers the user asked for. Replaces the `(bool, bool, bool,
/// bool)` tuple that used to be threaded through four call sites and rebuilt inline in a
/// fifth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltinFlags {
    pub terminal: bool,
    pub neovim: bool,
    pub time: bool,
    pub ui: bool,
}

impl BuiltinFlags {
    pub const ALL: Self = Self {
        terminal: true,
        neovim: true,
        time: true,
        ui: true,
    };

    pub fn any(self) -> bool {
        self.terminal || self.neovim || self.time || self.ui
    }
}

/// Names opman ships. The settings page marks these as toggleable but not removable:
/// deleting the config entry only restores the built-in.
pub const BUILTIN_NAMES: [&str; 9] = [
    "terminal",
    "neovim",
    "time",
    "ui",
    "skills",
    "kanban",
    "agent-manager",
    "ask",
    "browser",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

/// opman's servers for these flags, in stable order.
///
/// `terminal`/`neovim`/`time`/`ui` appear only when flagged. `kanban`, `ask` and
/// `agent-manager` are unconditional but carry a [`Presence`] that is re-checked at
/// bind — which is what preserves the Claude engine's ability to pick up kanban after
/// the web server writes `internal.json` partway through a run.
pub fn servers(exe: &str, flags: BuiltinFlags) -> Vec<ServerSpec> {
    let session = || (SESSION_ENV.into(), Arg::SessionId);
    let mut specs = Vec::with_capacity(BUILTIN_NAMES.len());

    if flags.terminal {
        specs.push(ServerSpec::stdio(
            "terminal",
            exe,
            vec![Arg::lit("mcp"), Arg::Dir],
            vec![session()],
        ));
    }
    if flags.neovim {
        specs.push(ServerSpec::stdio(
            "neovim",
            exe,
            vec![Arg::lit("mcp-nvim"), Arg::Dir],
            vec![session()],
        ));
    }
    if flags.time {
        specs.push(ServerSpec::stdio(
            "time",
            exe,
            vec![Arg::lit("mcp-time")],
            Vec::new(),
        ));
    }
    if flags.ui {
        specs.push(ServerSpec::stdio(
            "ui",
            exe,
            vec![Arg::lit("mcp-ui")],
            Vec::new(),
        ));
    }
    // Always offered: skills reaching no runner at all is the bug this fixes, and the
    // server answers gracefully when there are none.
    specs.push(ServerSpec::stdio(
        "skills",
        exe,
        vec![Arg::lit("mcp-skills")],
        Vec::new(),
    ));
    specs.push(
        ServerSpec::stdio("kanban", exe, vec![Arg::lit("mcp-kanban")], Vec::new())
            .with_presence(Presence::LoopbackDescriptor),
    );
    // Asking the user a question is the one thing no harness exposes the same way — ACP
    // has no primitive for it and Claude's ACP adapter disables its own tool outright.
    // Routing it through MCP is what makes it work identically on every runner. The
    // timeout is the proxy's: the call is held open for as long as the human takes.
    specs.push(
        ServerSpec::stdio(
            "ask",
            exe,
            vec![Arg::lit("mcp-ask"), Arg::Dir],
            vec![session()],
        )
        .with_presence(Presence::LoopbackDescriptor)
        .with_timeout(PROXY_TIMEOUT_SECS),
    );
    // Browser panes. Unconditional but loopback-gated like kanban: the tools act on tabs
    // the web server owns, so without it there is nothing to drive. A page load plus its
    // outline can outrun a default MCP timeout on a slow site, hence the proxy timeout.
    specs.push(
        ServerSpec::stdio(
            "browser",
            exe,
            vec![Arg::lit("mcp-browser"), Arg::Dir],
            Vec::new(),
        )
            .with_presence(Presence::LoopbackDescriptor)
            .with_timeout(PROXY_TIMEOUT_SECS),
    );
    specs.push(
        ServerSpec::stdio(
            "agent-manager",
            exe,
            vec![Arg::lit("mcp-agent-manager"), Arg::Dir],
            vec![
                session(),
                (MANAGER_SOCKET.into(), Arg::Env(MANAGER_SOCKET.into())),
            ],
        )
        .with_presence(Presence::Env(MANAGER_SOCKET.into())),
    );
    specs
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod builtin_tests;
