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

/// Built-ins that stay off until the user turns them on in Settings. Each one changes how the
/// agent works (a second shell, a Neovim editing path), so neither is something to switch on
/// for someone who did not ask.
const OFF_UNTIL_ENABLED: [&str; 2] = ["terminal", "neovim"];

/// Whether a built-in is absent until the user enables it.
pub fn is_off_until_enabled(name: &str) -> bool {
    OFF_UNTIL_ENABLED.contains(&name)
}

/// Names opman ships. The settings page marks these as toggleable but not removable:
/// deleting the config entry only restores the built-in.
pub const BUILTIN_NAMES: [&str; 12] = [
    "terminal",
    "neovim",
    "time",
    "ui",
    "skills",
    "kanban",
    "agent-manager",
    "ask",
    "browser",
    "opman-tools",
    "code",
    "opman-management",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

/// Every server opman ships, in stable order. Which of them a session gets is decided
/// by the user's `mcp.json` (the Settings page writes it) and by [`is_off_until_enabled`],
/// never by a launch flag.
///
/// `kanban`, `ask` and `agent-manager` carry a [`Presence`] that is re-checked at
/// bind — which is what preserves the Claude engine's ability to pick up kanban after
/// the web server writes `internal.json` partway through a run.
pub fn servers(exe: &str) -> Vec<ServerSpec> {
    let session = || (SESSION_ENV.into(), Arg::SessionId);
    let mut specs = Vec::with_capacity(BUILTIN_NAMES.len());
    specs.push(
        ServerSpec::stdio(
            "opman-management",
            exe,
            vec![Arg::lit("mcp-management")],
            Vec::new(),
        )
        .with_presence(Presence::LoopbackDescriptor)
        .with_timeout(PROXY_TIMEOUT_SECS),
    );

    specs.push(ServerSpec::stdio(
        "terminal",
        exe,
        vec![Arg::lit("mcp"), Arg::Dir],
        vec![session()],
    ));
    specs.push(ServerSpec::stdio(
        "neovim",
        exe,
        vec![Arg::lit("mcp-nvim"), Arg::Dir],
        vec![session()],
    ));
    specs.push(ServerSpec::stdio(
        "time",
        exe,
        vec![Arg::lit("mcp-time")],
        Vec::new(),
    ));
    // The long ceiling is for scripts that wait on slow tools.
    specs.push(
        ServerSpec::stdio(
            "code",
            exe,
            vec![Arg::lit("mcp-code"), Arg::Dir],
            vec![session()],
        )
        .with_timeout(PROXY_TIMEOUT_SECS),
    );
    specs.push(ServerSpec::stdio(
        "ui",
        exe,
        vec![Arg::lit("mcp-ui")],
        Vec::new(),
    ));
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
    // Common coding tools (bash, files, search, web fetch). Unconditional and harmless: the
    // tools only act when an agent calls them, and the user can switch the server off with
    // `{"opman-tools": {"enabled": false}}`. A command may run for minutes, so it gets the
    // long ceiling; it also sends progress, which keeps OpenCode's 60 s clock alive.
    specs.push(
        ServerSpec::stdio(
            "opman-tools",
            exe,
            vec![Arg::lit("mcp-tools"), Arg::Dir],
            Vec::new(),
        )
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
