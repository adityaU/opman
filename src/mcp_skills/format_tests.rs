//! Reading and writing SKILL.md, and the round trip that used to silently lose skills.

use super::*;

fn name(raw: &str) -> SkillName {
    SkillName::parse(raw).expect("valid name")
}

fn draft<'a>(n: &'a SkillName, description: &'a str, body: &'a str) -> SkillDraft<'a> {
    SkillDraft {
        name: n,
        title: None,
        description,
        requires: &[],
        body,
    }
}

#[test]
fn a_basic_file_parses() {
    let raw = "---\nname: Jira Triage\ndescription: Triage the backlog.\n---\n\nStep one.\n";
    let skill = parse_skill_md(raw, &name("jira")).expect("parses");
    assert_eq!(skill.name.as_str(), "jira");
    assert_eq!(skill.title, "Jira Triage");
    assert_eq!(skill.description, "Triage the backlog.");
    assert_eq!(skill.content, "Step one.");
}

/// Identity comes from the directory, not the frontmatter. Keying by the frontmatter
/// `name` meant `skills delete` could not find a skill `skills list` had just printed.
#[test]
fn the_directory_name_is_authoritative() {
    let raw = "---\nname: Something Else\ndescription: d\n---\nbody\n";
    let skill = parse_skill_md(raw, &name("actual-dir")).expect("parses");
    assert_eq!(skill.name.as_str(), "actual-dir");
    assert_eq!(skill.title, "Something Else");
}

#[test]
fn a_missing_frontmatter_name_falls_back_to_the_directory() {
    // Two nameless skills used to collide on the empty-string key and shadow each other.
    let raw = "---\ndescription: d\n---\nbody\n";
    let skill = parse_skill_md(raw, &name("dir-name")).expect("parses");
    assert_eq!(skill.title, "dir-name");
}

/// The old parser split on `---` anywhere, so a body containing a horizontal rule was
/// read as frontmatter.
#[test]
fn a_body_containing_a_rule_is_not_frontmatter() {
    let raw = "---\nname: n\ndescription: d\n---\nintro\n\n---\n\nmore body\n";
    let skill = parse_skill_md(raw, &name("x")).expect("parses");
    assert!(skill.content.contains("intro"));
    assert!(skill.content.contains("more body"));
    assert!(skill.content.contains("---"));
}

#[test]
fn a_file_without_frontmatter_is_rejected() {
    assert!(parse_skill_md("just a body\n", &name("x")).is_err());
    assert!(parse_skill_md("---\nname: n\nno close delimiter\n", &name("x")).is_err());
}

/// The bug this file exists to kill: the old `format!` template interpolated the
/// description straight into YAML, so a `:` produced frontmatter the parser rejected —
/// the write reported success and the skill never appeared in any listing.
#[test]
fn hostile_descriptions_round_trip() {
    for description in [
        "Fix: the thing",
        "line one\nline two",
        "- leading dash",
        "# hash",
        "quotes \"inside\" it",
        "trailing colon:",
    ] {
        let n = name("x");
        let rendered = render_skill_md(&draft(&n, description, "body")).expect("renders");
        let parsed = parse_skill_md(&rendered, &n)
            .unwrap_or_else(|e| panic!("{description:?} failed to round trip: {e}"));
        assert_eq!(parsed.description, description, "for {description:?}");
    }
}

#[test]
fn a_body_containing_a_rule_round_trips() {
    let n = name("x");
    let rendered = render_skill_md(&draft(&n, "d", "before\n\n---\n\nafter")).expect("renders");
    let parsed = parse_skill_md(&rendered, &n).expect("parses");
    assert!(parsed.content.contains("before"));
    assert!(parsed.content.contains("after"));
}

#[test]
fn requires_accepts_a_list_or_a_bare_string() {
    let list = parse_skill_md(
        "---\ndescription: d\nrequires: [jira, linear]\n---\nb\n",
        &name("x"),
    )
    .expect("parses");
    assert_eq!(list.requires, ["jira", "linear"]);

    let one = parse_skill_md("---\ndescription: d\nrequires: jira\n---\nb\n", &name("x"))
        .expect("parses");
    assert_eq!(one.requires, ["jira"]);

    let none = parse_skill_md("---\ndescription: d\n---\nb\n", &name("x")).expect("parses");
    assert!(none.requires.is_empty());
}

#[test]
fn requires_round_trips() {
    let n = name("x");
    let requires = vec!["jira".to_string()];
    let rendered = render_skill_md(&SkillDraft {
        name: &n,
        title: None,
        description: "d",
        requires: &requires,
        body: "b",
    })
    .expect("renders");
    assert_eq!(
        parse_skill_md(&rendered, &n).expect("parses").requires,
        requires
    );
}
