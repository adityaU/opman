use super::*;
use std::fs;
use tempfile::TempDir;

/// Write a file, creating its parents.
fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parents");
    }
    fs::write(path, body).expect("write file");
}

fn frontmatter(description: &str) -> String {
    format!("---\nname: x\ndescription: {description}\n---\n\nBody text.\n")
}

#[test]
fn command_file_is_named_by_its_path_below_commands() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        ".claude/commands/deploy.md",
        &frontmatter("Ship it"),
    );
    write(
        dir.path(),
        ".claude/commands/git/sync.md",
        &frontmatter("Sync the fork"),
    );

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    assert_eq!(lookup(&found, "deploy"), Some("Ship it"));
    assert_eq!(lookup(&found, "git:sync"), Some("Sync the fork"));
}

#[test]
fn skill_is_named_by_its_directory() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        ".claude/skills/impeccable/SKILL.md",
        &frontmatter("Polish an interface"),
    );

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    assert_eq!(lookup(&found, "impeccable"), Some("Polish an interface"));
}

#[test]
fn namespaced_name_falls_back_to_the_bare_definition() {
    // Plugins report `plugin:skill`; the file on disk only knows its own name.
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        ".claude/skills/frontend-design/SKILL.md",
        &frontmatter("Visual direction"),
    );

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    assert_eq!(
        lookup(&found, "frontend-design:frontend-design"),
        Some("Visual direction")
    );
}

#[test]
fn a_command_with_no_file_has_no_description() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        ".claude/commands/deploy.md",
        &frontmatter("Ship"),
    );

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    // `compact` is a claude built-in: no file, so nothing invented for it.
    assert_eq!(lookup(&found, "compact"), None);
}

#[test]
fn quoted_and_empty_descriptions() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        ".claude/commands/quoted.md",
        "---\ndescription: \"Quoted prose\"\n---\n",
    );
    write(
        dir.path(),
        ".claude/commands/blank.md",
        "---\ndescription:\n---\n",
    );
    write(dir.path(), ".claude/commands/none.md", "Just a body.\n");

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    assert_eq!(lookup(&found, "quoted"), Some("Quoted prose"));
    assert_eq!(lookup(&found, "blank"), None);
    assert_eq!(lookup(&found, "none"), None);
}

#[test]
fn description_after_the_frontmatter_is_not_read() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        ".claude/commands/prose.md",
        "---\nname: prose\n---\n\ndescription: not frontmatter\n",
    );

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    assert_eq!(lookup(&found, "prose"), None);
}

#[test]
fn a_missing_root_is_not_an_error() {
    let dir = TempDir::new().expect("tempdir");

    let found = describe(dir.path().to_str().expect("utf-8 path"));

    assert!(found.iter().all(|(name, _)| name != "deploy"));
}
