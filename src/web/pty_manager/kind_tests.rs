use super::*;

/// The browser sends these strings; a rename here silently breaks the UI.
#[test]
fn kinds_deserialize_from_their_wire_names() {
    let cases = [
        ("\"shell\"", PtyKind::Shell),
        ("\"neovim\"", PtyKind::Neovim),
        ("\"git\"", PtyKind::Git),
        ("\"opencode\"", PtyKind::Opencode),
        ("\"claude-attach\"", PtyKind::ClaudeAttach),
    ];
    for (json, expected) in cases {
        let parsed: PtyKind = serde_json::from_str(json).expect("wire name should parse");
        assert_eq!(parsed, expected, "{json}");
        let encoded = serde_json::to_string(&expected).expect("a kind serializes");
        assert_eq!(encoded, json, "round trip");
    }
}

#[test]
fn unknown_kind_is_refused_rather_than_defaulted() {
    assert!(serde_json::from_str::<PtyKind>("\"powershell\"").is_err());
}

#[test]
fn program_reports_its_kind() {
    assert_eq!(PtyProgram::Shell.kind(), PtyKind::Shell);
    assert_eq!(
        PtyProgram::Opencode { session_id: None }.kind(),
        PtyKind::Opencode
    );
    assert_eq!(
        PtyProgram::ClaudeAttach {
            short_id: "abc".into()
        }
        .kind(),
        PtyKind::ClaudeAttach
    );
}

#[test]
fn labels_are_how_the_kind_reads_to_a_user() {
    assert_eq!(PtyKind::Shell.label(), "Shell");
    assert_eq!(PtyKind::Opencode.label(), "OpenCode");
    assert_eq!(PtyKind::ClaudeAttach.label(), "Claude");
}
