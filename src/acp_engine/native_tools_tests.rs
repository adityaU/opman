//! The choice type, its file format, and the catalogue's shape.

use super::super::native_tools_catalog::tool_set_for;
use super::super::patch::AcpDocument;
use super::super::{catalog, config};
use super::{NativeTools, ToolSet};

fn claude() -> &'static ToolSet {
    tool_set_for("claude").expect("claude has tools")
}

fn disabled(ids: &[&str]) -> NativeTools {
    NativeTools::Disabled(ids.iter().map(|id| (*id).to_string()).collect())
}

#[test]
fn a_file_without_the_key_means_everything_is_on() {
    let legacy = r#"{"agents":{"claude":{"enabled":true,"args":["x"]}}}"#;
    let document: AcpDocument = serde_json::from_str(legacy).expect("legacy file loads");
    let cfg = config::resolve(document);
    assert_eq!(cfg.agents["claude"].native_tools, NativeTools::All);
}

#[test]
fn every_spelling_round_trips() {
    for setting in [
        NativeTools::All,
        NativeTools::Off,
        disabled(&["Bash", "Read"]),
    ] {
        let text = serde_json::to_string(&setting).expect("serialize");
        let back: NativeTools = serde_json::from_str(&text).expect("parse");
        assert_eq!(back, setting, "{text}");
    }
    assert_eq!(
        serde_json::to_string(&NativeTools::Off).expect("ser"),
        r#""off""#
    );
    assert_eq!(
        serde_json::to_string(&disabled(&["Bash"])).expect("ser"),
        r#"{"disabled":["Bash"]}"#
    );
}

#[test]
fn a_saved_choice_reaches_the_resolved_config() {
    let text = r#"{"agents":{"claude":{"nativeTools":{"disabled":["Bash"]}}}}"#;
    let document: AcpDocument = serde_json::from_str(text).expect("parse");
    let cfg = config::resolve(document);
    let claude = &cfg.agents["claude"];
    assert!(claude.native_tools.turns_off("Bash"));
    assert!(!claude.native_tools.turns_off("Read"));
    assert!(claude.tool_set.is_some());
}

#[test]
fn off_and_all_cover_the_whole_catalogue() {
    let set = claude();
    assert!(NativeTools::All.disabled(set).is_empty());
    assert_eq!(NativeTools::Off.disabled(set).len(), set.tools.len());
}

#[test]
fn disabled_ignores_ids_the_harness_does_not_have() {
    let picked = disabled(&["Bash", "NoSuchTool"]);
    let ids: Vec<&str> = picked.disabled(claude()).iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["Bash"]);
}

#[test]
fn canonical_has_one_spelling_per_state() {
    let set = claude();
    assert_eq!(disabled(&[]).canonical(set), NativeTools::All);
    assert_eq!(disabled(&["Nope"]).canonical(set), NativeTools::All);
    let every: Vec<&str> = set.tools.iter().map(|t| t.id).collect();
    assert_eq!(disabled(&every).canonical(set), NativeTools::Off);
    assert_eq!(
        disabled(&["Bash", "Nope"]).canonical(set),
        disabled(&["Bash"])
    );
    assert_eq!(NativeTools::Off.canonical(set), NativeTools::Off);
}

#[test]
fn only_harnesses_with_a_tested_mechanism_have_tools() {
    for id in ["claude", "codex", "opencode", "antigravity"] {
        assert!(tool_set_for(id).is_some(), "{id}");
    }
    assert!(tool_set_for("gemini").is_none());
    assert!(tool_set_for("homegrown").is_none());
}

#[test]
fn every_catalogue_id_is_unique_within_its_harness() {
    for id in ["claude", "codex", "opencode", "antigravity"] {
        let set = tool_set_for(id).expect("set");
        let mut ids: Vec<&str> = set.tools.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), set.tools.len(), "{id}");
        assert!(set
            .tools
            .iter()
            .all(|t| !t.label.is_empty() && !t.description.is_empty()));
    }
}

#[test]
fn builtin_agents_get_their_tool_set_and_others_do_not() {
    let cfg = config::resolve(AcpDocument::default());
    assert!(cfg.agents["claude"].tool_set.is_some());
    assert!(cfg.agents["opencode"].tool_set.is_some());
    assert!(catalog::is_builtin("gemini"));
    assert!(cfg.agents["gemini"].tool_set.is_none());
}

#[test]
fn an_agent_the_user_declared_is_found_by_the_program_it_runs() {
    use super::super::native_tools_catalog::tool_set_for_agent;
    let opencode = tool_set_for_agent("opencode-acp", "/home/u/.opencode/bin/opencode");
    assert_eq!(opencode, tool_set_for("opencode"));
    assert!(opencode.is_some());
    assert!(tool_set_for_agent("homegrown", "/bin/homegrown").is_none());
    assert!(tool_set_for_agent("x", "").is_none());
}

#[test]
fn a_declared_opencode_agent_gets_the_switch_in_its_resolved_config() {
    let document: AcpDocument = serde_json::from_str(
        r#"{"agents":{"opencode-acp":{"command":"/home/u/.opencode/bin/opencode","args":["acp"]}}}"#,
    )
    .expect("valid acp.json");
    let cfg = config::resolve(document);
    assert!(cfg.agents["opencode-acp"].tool_set.is_some());
}
