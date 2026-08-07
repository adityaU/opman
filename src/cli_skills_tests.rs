//! The CLI skill paths, and the traversal they used to allow.

use super::*;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("opman-cli-skills-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    root
}

#[test]
fn traversal_names_are_rejected_before_touching_the_filesystem() {
    // `opman skills delete ../..` used to reach std::fs::remove_dir_all directly.
    for raw in ["..", "../..", "a/b", "/etc"] {
        assert!(parse(raw).is_err(), "{raw:?} must not parse");
    }
}

#[test]
fn a_written_skill_round_trips_through_the_parser() {
    let root = temp_root("roundtrip");
    let name = SkillName::parse("demo").expect("valid");
    let dir = name.dir_in(&root);
    std::fs::create_dir_all(&dir).expect("create");
    // A description with a colon is exactly what the old format! template broke on.
    write(&dir, &name, "Fix: the thing", "BODY").expect("write");

    let raw = std::fs::read_to_string(dir.join("SKILL.md")).expect("read");
    let skill = crate::mcp_skills::format::parse_skill_md(&raw, &name).expect("parses");
    assert_eq!(skill.description, "Fix: the thing");
    assert_eq!(skill.content, "BODY");
    let _ = std::fs::remove_dir_all(&root);
}
