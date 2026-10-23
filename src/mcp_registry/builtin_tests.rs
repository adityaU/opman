//! opman's own servers. These assertions are the behaviour contract carried over from
//! the four hand-rolled injection functions this module replaced.

use super::*;
use crate::mcp_registry::spec::Transport;

fn names() -> Vec<String> {
    servers("/opman")
        .iter()
        .map(|spec| spec.name().to_string())
        .collect()
}

/// Every built-in is always declared. `kanban`, `ask`, `browser` and `agent-manager` are
/// gated at bind time; `terminal` and `neovim` are held back by the registry until enabled.
#[test]
fn every_builtin_is_declared_in_a_stable_order() {
    assert_eq!(
        names(),
        [
            "opman-management",
            "terminal",
            "neovim",
            "time",
            "code",
            "ui",
            "skills",
            "kanban",
            "ask",
            "browser",
            "opman-tools",
            "agent-manager"
        ]
    );
}

#[test]
fn only_terminal_and_neovim_are_off_until_enabled() {
    let off: Vec<String> = names()
        .into_iter()
        .filter(|name| is_off_until_enabled(name))
        .collect();
    assert_eq!(off, ["terminal", "neovim"]);
}

/// Browser panes live in the web server, so the tools are worth offering only when it is
/// there to own them — the same bind-time gate kanban uses.
#[test]
fn browser_is_gated_on_the_loopback_descriptor_at_bind_time() {
    let specs = servers("/opman");
    let browser = specs
        .iter()
        .find(|spec| spec.name() == "browser")
        .expect("browser is a built-in");
    assert_eq!(browser.presence, Presence::LoopbackDescriptor);
    // A page load plus its outline can outrun a default MCP timeout on a slow site.
    assert_eq!(browser.timeout_secs, Some(PROXY_TIMEOUT_SECS));
}

#[test]
fn kanban_is_gated_on_the_loopback_descriptor_at_bind_time() {
    // Bind-time rather than load-time is what lets a runner pick kanban up after the
    // web server starts, without restarting opman.
    let specs = servers("/opman");
    let kanban = specs.iter().find(|s| s.name() == "kanban").expect("kanban");
    assert_eq!(kanban.presence, Presence::LoopbackDescriptor);
}

/// The question server is the one built-in whose call is held open by a human, so it needs
/// both halves of the timeout story: the loopback it dials and a ceiling far above the
/// 60s a silent call gets by default.
#[test]
fn ask_is_gated_on_the_loopback_and_carries_a_long_ceiling() {
    let specs = servers("/opman");
    let ask = specs.iter().find(|s| s.name() == "ask").expect("ask");
    assert_eq!(ask.presence, Presence::LoopbackDescriptor);
    assert_eq!(ask.timeout_secs(), Some(PROXY_TIMEOUT_SECS));
    // It routes by session, and takes the project directory as the fallback for a runner
    // whose config is process-wide and cannot supply one.
    assert!(ask.binds_session());
    let Transport::Stdio(stdio) = ask.transport() else {
        panic!("ask should be stdio");
    };
    assert_eq!(stdio.args[0], Arg::lit("mcp-ask"));
    assert_eq!(stdio.args[1], Arg::Dir);
}

#[test]
fn agent_manager_is_gated_on_the_socket_variable() {
    let specs = servers("/opman");
    let manager = specs
        .iter()
        .find(|s| s.name() == "agent-manager")
        .expect("agent-manager");
    assert_eq!(manager.presence, Presence::Env(MANAGER_SOCKET.into()));
    assert!(manager.binds_session());
}

#[test]
fn the_bridges_that_route_by_session_declare_the_session_variable() {
    let specs = servers("/opman");
    for name in ["terminal", "neovim", "agent-manager", "ask"] {
        let spec = specs.iter().find(|s| s.name() == name).expect(name);
        assert!(spec.binds_session(), "{name} should carry the session id");
    }
    // time and ui do not read it, so they do not carry it.
    for name in ["time", "ui"] {
        let spec = specs.iter().find(|s| s.name() == name).expect(name);
        assert!(
            !spec.binds_session(),
            "{name} should not carry a session id"
        );
    }
}

#[test]
fn the_project_bridges_take_the_directory_positionally() {
    let specs = servers("/opman");
    for (name, subcommand) in [
        ("terminal", "mcp"),
        ("neovim", "mcp-nvim"),
        ("agent-manager", "mcp-agent-manager"),
        ("ask", "mcp-ask"),
    ] {
        let spec = specs.iter().find(|s| s.name() == name).expect(name);
        let Transport::Stdio(stdio) = spec.transport() else {
            panic!("{name} should be stdio");
        };
        assert_eq!(stdio.args[0], Arg::lit(subcommand));
        assert_eq!(stdio.args[1], Arg::Dir);
    }
}

#[test]
fn skills_is_always_offered() {
    let specs = servers("/opman");
    let skills = specs.iter().find(|s| s.name() == "skills").expect("skills");
    assert_eq!(skills.presence, Presence::Always);
    assert!(!skills.binds_session());
}

#[test]
fn every_builtin_launches_opman_itself() {
    for spec in servers("/opman/bin") {
        let Transport::Stdio(stdio) = spec.transport() else {
            panic!("built-ins are all stdio");
        };
        assert_eq!(&*stdio.command, "/opman/bin");
    }
}

/// The coding tools are on by default, take the project directory, and carry the long
/// ceiling because a shell command may run for minutes.
#[test]
fn opman_tools_is_always_on_and_takes_the_project_directory() {
    let specs = servers("/opman");
    let tools = specs
        .iter()
        .find(|s| s.name() == "opman-tools")
        .expect("opman-tools is a built-in");
    assert_eq!(tools.presence, Presence::Always);
    assert!(!tools.binds_session());
    assert_eq!(tools.timeout_secs(), Some(PROXY_TIMEOUT_SECS));
    let Transport::Stdio(stdio) = tools.transport() else {
        panic!("opman-tools should be stdio");
    };
    assert_eq!(stdio.args[0], Arg::lit("mcp-tools"));
    assert_eq!(stdio.args[1], Arg::Dir);
    assert!(crate::mcp_registry::builtin::is_builtin("opman-tools"));
}
