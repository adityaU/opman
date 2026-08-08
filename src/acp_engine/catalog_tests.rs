use super::*;

/// Ids are runner slots, and a runner slot is what every persisted session names. Two rows
/// sharing one would make which engine serves a session depend on map iteration order.
#[test]
fn ids_are_unique() {
    let mut ids: Vec<&str> = ENTRIES.iter().map(|entry| entry.id).collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate agent id in the catalogue");
}

/// The catalogue is the whole published agent list, but a fresh install must still spawn
/// only the two agents opman is developed against.
#[test]
fn only_claude_and_codex_ship_enabled() {
    let enabled: Vec<&str> = ENTRIES
        .iter()
        .filter(|entry| entry.config().enabled)
        .map(|entry| entry.id)
        .collect();
    assert_eq!(enabled, vec!["claude", "codex"]);
}

/// A row with no documented command is declared, not launchable — the settings page shows
/// it so the command can be filled in, and nothing tries to spawn an empty program name.
#[test]
fn undocumented_rows_are_declared_but_not_launchable() {
    let entry = ENTRIES
        .iter()
        .find(|entry| entry.id == "gemini")
        .expect("gemini is catalogued");
    let config = entry.config();
    assert!(config.command.is_empty());
    assert!(!config.launchable());
    assert!(
        !entry.docs.is_empty(),
        "an undocumented row still links docs"
    );
}

/// Every row is reachable by id, which is what the settings page uses to decide whether a
/// Remove deletes an agent or restores opman's definition.
#[test]
fn every_row_is_builtin_and_documented() {
    for entry in ENTRIES {
        assert!(is_builtin(entry.id), "{} is not builtin", entry.id);
        assert_eq!(docs_for(entry.id), Some(entry.docs));
        assert!(!entry.name.is_empty());
    }
    assert!(!is_builtin("something-the-user-declared"));
    assert_eq!(docs_for("something-the-user-declared"), None);
}

/// The launch kinds each resolve to the command shape the engine spawns.
#[test]
fn launch_kinds_resolve() {
    let npm = ENTRIES
        .iter()
        .find(|entry| entry.id == "claude")
        .expect("claude is catalogued")
        .config();
    assert_eq!(npm.args.first().map(String::as_str), Some("-y"));

    let uvx = ENTRIES
        .iter()
        .find(|entry| entry.id == "agentpool")
        .expect("agentpool is catalogued")
        .config();
    assert_eq!(uvx.command, "uvx");
    assert_eq!(uvx.args, vec!["agentpool@latest", "serve-acp"]);

    let bin = ENTRIES
        .iter()
        .find(|entry| entry.id == "goose")
        .expect("goose is catalogued")
        .config();
    assert_eq!(bin.command, "goose");
    assert_eq!(bin.args, vec!["acp"]);
    // An unset runner slot defaults to the id, so the catalogue never spells it twice.
    assert_eq!(bin.runner, "goose");
}
