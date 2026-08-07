//! Name validation. These are the traversal guards: `SkillName` is the only thing a
//! path is built from, so what it rejects can never reach the filesystem.

use super::*;

#[test]
fn ordinary_names_parse() {
    assert_eq!(SkillName::parse("jira-triage").expect("ok").as_str(), "jira-triage");
    assert_eq!(SkillName::parse("a").expect("ok").as_str(), "a");
    assert_eq!(SkillName::parse("v1.2_x").expect("ok").as_str(), "v1.2_x");
}

#[test]
fn names_are_case_folded() {
    assert_eq!(SkillName::parse("JiraTriage").expect("ok").as_str(), "jiratriage");
}

#[test]
fn traversal_is_rejected() {
    for raw in ["..", ".", "../evil", "a/b", "a\\b", "/etc/passwd", "./x"] {
        assert!(
            SkillName::parse(raw).is_err(),
            "{raw:?} must not parse into a skill name"
        );
    }
}

#[test]
fn a_leading_dot_is_rejected() {
    assert_eq!(SkillName::parse(".hidden"), Err(SkillNameError::BadChar('.')));
}

#[test]
fn empty_and_overlong_names_are_rejected() {
    assert_eq!(SkillName::parse(""), Err(SkillNameError::Empty));
    assert_eq!(SkillName::parse("   "), Err(SkillNameError::Empty));
    assert_eq!(SkillName::parse(&"a".repeat(65)), Err(SkillNameError::TooLong));
    assert!(SkillName::parse(&"a".repeat(64)).is_ok());
}

#[test]
fn a_nul_byte_is_rejected() {
    assert!(SkillName::parse("a\0b").is_err());
}

#[test]
fn dir_in_never_escapes_its_root() {
    let root = std::path::Path::new("/skills");
    for raw in ["ok", "a-b", "x.y"] {
        let name = SkillName::parse(raw).expect("valid");
        assert!(name.dir_in(root).starts_with(root));
    }
}

#[test]
fn deserialize_validates_on_the_way_in() {
    // The whole point of the newtype: a request body cannot carry an invalid name into
    // a handler that then has to remember to check it.
    assert!(serde_json::from_str::<SkillName>("\"ok\"").is_ok());
    assert!(serde_json::from_str::<SkillName>("\"../evil\"").is_err());
}
