//! opman's own servers. These assertions are the behaviour contract carried over from
//! the four hand-rolled injection functions this module replaced.

use super::*;
use crate::mcp_registry::spec::Transport;

fn names(flags: BuiltinFlags) -> Vec<String> {
    servers("/opman", flags)
        .iter()
        .map(|spec| spec.name().to_string())
        .collect()
}

/// `skills` is unflagged: skills reaching no runner at all is the bug this fixes.
/// `kanban` and `agent-manager` are unconditional in the list and gated at bind time.
#[test]
fn no_flags_still_yields_the_unflagged_servers() {
    assert_eq!(
        names(BuiltinFlags::default()),
        ["skills", "kanban", "agent-manager"]
    );
}

#[test]
fn every_flag_adds_exactly_its_own_server() {
    let flags = BuiltinFlags {
        terminal: true,
        ..BuiltinFlags::default()
    };
    assert_eq!(
        names(flags),
        ["terminal", "skills", "kanban", "agent-manager"]
    );
}

#[test]
fn all_flags_yield_every_builtin() {
    assert_eq!(
        names(BuiltinFlags::ALL),
        [
            "terminal",
            "neovim",
            "time",
            "ui",
            "skills",
            "kanban",
            "agent-manager"
        ]
    );
}

#[test]
fn kanban_is_gated_on_the_loopback_descriptor_at_bind_time() {
    // Bind-time rather than load-time is what lets a runner pick kanban up after the
    // web server starts, without restarting opman.
    let specs = servers("/opman", BuiltinFlags::default());
    let kanban = specs.iter().find(|s| s.name() == "kanban").expect("kanban");
    assert_eq!(kanban.presence, Presence::KanbanDescriptor);
}

#[test]
fn agent_manager_is_gated_on_the_socket_variable() {
    let specs = servers("/opman", BuiltinFlags::default());
    let manager = specs
        .iter()
        .find(|s| s.name() == "agent-manager")
        .expect("agent-manager");
    assert_eq!(manager.presence, Presence::Env(MANAGER_SOCKET.into()));
    assert!(manager.binds_session());
}

#[test]
fn the_bridges_that_route_by_session_declare_the_session_variable() {
    let specs = servers("/opman", BuiltinFlags::ALL);
    for name in ["terminal", "neovim", "agent-manager"] {
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
    let specs = servers("/opman", BuiltinFlags::ALL);
    for (name, subcommand) in [
        ("terminal", "mcp"),
        ("neovim", "mcp-nvim"),
        ("agent-manager", "mcp-agent-manager"),
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
fn skills_is_offered_without_a_flag() {
    let specs = servers("/opman", BuiltinFlags::default());
    let skills = specs.iter().find(|s| s.name() == "skills").expect("skills");
    assert_eq!(skills.presence, Presence::Always);
    assert!(!skills.binds_session());
}

#[test]
fn every_builtin_launches_opman_itself() {
    for spec in servers("/opman/bin", BuiltinFlags::ALL) {
        let Transport::Stdio(stdio) = spec.transport() else {
            panic!("built-ins are all stdio");
        };
        assert_eq!(&*stdio.command, "/opman/bin");
    }
}

#[test]
fn flags_any_reports_whether_anything_was_asked_for() {
    assert!(!BuiltinFlags::default().any());
    assert!(BuiltinFlags::ALL.any());
}
