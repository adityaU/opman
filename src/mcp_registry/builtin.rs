//! opman's own MCP servers, expressed as ordinary [`ServerSpec`]s.
//!
//! Seeding them into the same map the user's `mcp.json` merges over is what lets a
//! one-line override disable or retune a built-in, and it collapses the gating that used
//! to be duplicated across four injection sites — three separate `internal.json`
//! existence checks and three separate socket-env checks — into one declaration each.

use super::spec::{Arg, Presence, ServerSpec};

/// The environment variable the terminal, neovim, and agent-manager bridges read to
/// route back to the session that launched them.
const SESSION_ENV: &str = "OPENCODE_SESSION_ID";
/// Set by opman before any runner is spawned; its presence is what says the in-process
/// agent-manager listener exists to talk to.
const MANAGER_SOCKET: &str = "OPMAN_AGENT_MANAGER_SOCKET";

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
pub const BUILTIN_NAMES: [&str; 7] = [
    "terminal",
    "neovim",
    "time",
    "ui",
    "skills",
    "kanban",
    "agent-manager",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

/// opman's servers for these flags, in stable order.
///
/// `terminal`/`neovim`/`time`/`ui` appear only when flagged. `kanban` and
/// `agent-manager` are unconditional but carry a [`Presence`] that is re-checked at
/// bind — which is what preserves the Claude engine's ability to pick up kanban after
/// the web server writes `internal.json` partway through a run.
pub fn servers(exe: &str, flags: BuiltinFlags) -> Vec<ServerSpec> {
    let session = || (SESSION_ENV.into(), Arg::SessionId);
    let mut specs = Vec::with_capacity(6);

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
            .with_presence(Presence::KanbanDescriptor),
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
