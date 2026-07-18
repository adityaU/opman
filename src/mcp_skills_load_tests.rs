//! Wave-2 coverage for `load_skills_from` (directory walk + parse + filter).
use super::*;

#[test]
fn load_skills_from_creates_missing_dir_and_returns_empty() {
    let base = tempfile::TempDir::new().unwrap();
    let missing = base.path().join("does-not-exist-yet");
    assert!(!missing.exists());
    let out = load_skills_from(&missing).unwrap();
    assert!(out.is_empty());
    // The missing directory should have been created.
    assert!(missing.exists() && missing.is_dir());
}

#[test]
fn load_skills_from_reads_valid_skill() {
    let dir = tempfile::TempDir::new().unwrap();
    let skill_dir = dir.path().join("greeter");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: greeter\ndescription: says hi\n---\nSay hello to the user.",
    )
    .unwrap();

    let out = load_skills_from(dir.path()).unwrap();
    assert_eq!(out.len(), 1);
    let s = out.get("greeter").unwrap();
    assert_eq!(s.description, "says hi");
    assert_eq!(s.content, "Say hello to the user.");
}

#[test]
fn load_skills_from_skips_dir_without_skill_md() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("empty-skill")).unwrap();
    let out = load_skills_from(dir.path()).unwrap();
    assert!(out.is_empty());
}

#[test]
fn load_skills_from_skips_plain_files() {
    let dir = tempfile::TempDir::new().unwrap();
    // A regular file at the top level (not a directory) must be ignored.
    std::fs::write(dir.path().join("README.md"), "not a skill").unwrap();
    let out = load_skills_from(dir.path()).unwrap();
    assert!(out.is_empty());
}

#[test]
fn load_skills_from_skips_malformed_skill_md() {
    let dir = tempfile::TempDir::new().unwrap();
    let bad = dir.path().join("broken");
    std::fs::create_dir_all(&bad).unwrap();
    // Missing frontmatter → parse_skill errors → skill is not inserted.
    std::fs::write(bad.join("SKILL.md"), "no frontmatter here").unwrap();
    let out = load_skills_from(dir.path()).unwrap();
    assert!(out.is_empty());
}

#[test]
fn load_skills_from_multiple_skills() {
    let dir = tempfile::TempDir::new().unwrap();
    for (name, desc) in [("alpha", "a"), ("beta", "b")] {
        let d = dir.path().join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\nbody-{name}"),
        )
        .unwrap();
    }
    let out = load_skills_from(dir.path()).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.contains_key("alpha"));
    assert!(out.contains_key("beta"));
    assert_eq!(out["beta"].content, "body-beta");
}
