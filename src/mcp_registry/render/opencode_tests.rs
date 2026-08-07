//! OpenCode's `OPENCODE_CONFIG_CONTENT` shape, including the two things that make it
//! the odd one out: it merges into an existing config, and it has no session id.

use super::*;
use crate::mcp_registry::spec::Arg;

fn at<'a>() -> Bind<'a> {
    // Process-wide: the child's own working directory, and no session.
    Bind::new("/opman", ".", None)
}

fn parsed(specs: &[ServerSpec], flags: BuiltinFlags) -> serde_json::Value {
    let raw = config(specs.iter(), at(), flags).expect("payload");
    serde_json::from_str(&raw).expect("valid json")
}

#[test]
fn stdio_is_one_flat_command_array() {
    let spec = ServerSpec::stdio("terminal", "/opman", vec![Arg::lit("mcp"), Arg::Dir], Vec::new());
    let json = parsed(&[spec], BuiltinFlags::default());
    let command = &json["mcp"]["terminal"]["command"];
    assert_eq!(command[0], "/opman");
    assert_eq!(command[1], "mcp");
    assert_eq!(command[2], ".");
}

/// OpenCode merges this payload with its own config files, so opman emits only its own
/// keys. Adding anything else here would override a user setting for no reason.
#[test]
fn only_opmans_own_keys_are_emitted() {
    let json = parsed(&[], BuiltinFlags::default());
    let keys: Vec<_> = json.as_object().expect("object").keys().cloned().collect();
    assert_eq!(keys, ["mcp"]);
}

#[test]
fn the_terminal_bridge_denies_opencodes_own_bash() {
    let flags = BuiltinFlags {
        terminal: true,
        ..BuiltinFlags::default()
    };
    let json = parsed(&[], flags);
    assert_eq!(json["permission"]["bash"], "deny");
    assert!(json["permission"].get("edit").is_none());
}

#[test]
fn the_neovim_bridge_denies_opencodes_own_edit() {
    let flags = BuiltinFlags {
        neovim: true,
        ..BuiltinFlags::default()
    };
    let json = parsed(&[], flags);
    assert_eq!(json["permission"]["edit"], "deny");
    assert!(json["permission"].get("bash").is_none());
}

#[test]
fn no_bridges_means_no_permission_block_at_all() {
    let json = parsed(&[], BuiltinFlags::default());
    assert!(json.get("permission").is_none());
}

#[test]
fn a_server_needing_a_session_positionally_is_skipped_not_mangled() {
    let spec = ServerSpec::stdio("needs", "/opman", vec![Arg::SessionId], Vec::new());
    let json = parsed(&[spec], BuiltinFlags::default());
    assert!(json["mcp"].as_object().is_some_and(|m| m.is_empty()));
}

#[test]
fn the_timeout_is_emitted_in_milliseconds() {
    let mut spec = ServerSpec::stdio("x", "/opman", vec![Arg::lit("mcp")], Vec::new());
    spec.timeout_secs = Some(900);
    let json = parsed(&[spec], BuiltinFlags::default());
    assert_eq!(json["mcp"]["x"]["timeout"], 900_000);
}
