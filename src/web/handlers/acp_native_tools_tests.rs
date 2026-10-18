//! The list view of an agent's native tools.

use super::*;

fn claude(setting: NativeTools) -> AgentConfig {
    AgentConfig {
        native_tools: setting,
        tool_set: config::resolve(patch::AcpDocument::default()).agents["claude"].tool_set,
        ..AgentConfig::default()
    }
}

#[test]
fn an_agent_without_a_mechanism_has_no_view() {
    assert!(native_tools_view(&AgentConfig::default()).is_none());
}

#[test]
fn the_view_marks_each_tool_on_or_off() {
    let off = NativeTools::Disabled(["Bash".to_string()].into());
    let view = native_tools_view(&claude(off)).expect("view");
    let state: Vec<(&str, bool)> = view.tools.iter().map(|t| (t.id, t.enabled)).collect();
    assert!(state.contains(&("Bash", false)));
    assert!(state.contains(&("Read", true)));
    assert!(!view.note.is_empty());
}

#[test]
fn the_view_serializes_the_setting_the_page_sends_back() {
    let view = native_tools_view(&claude(NativeTools::Off)).expect("view");
    let json = serde_json::to_value(&view).expect("serialize");
    assert_eq!(json["setting"], "off");
    assert!(json["tools"]
        .as_array()
        .is_some_and(|t| t.iter().all(|t| t["enabled"] == false)));
}

#[test]
fn the_request_body_is_the_same_shape_as_the_file() {
    let body: SetNativeTools =
        serde_json::from_str(r#"{"setting":{"disabled":["Bash"]}}"#).expect("parse");
    assert_eq!(
        body.setting,
        NativeTools::Disabled(["Bash".to_string()].into())
    );
    let all: SetNativeTools = serde_json::from_str(r#"{"setting":"all"}"#).expect("parse");
    assert_eq!(all.setting, NativeTools::All);
}
