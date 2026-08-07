//! Directory scanning and the TTL re-scan that gives live edits without a restart.

use std::time::Duration;

use super::*;

fn write_skill(root: &std::path::Path, name: &str, description: &str, body: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n"),
    )
    .expect("write SKILL.md");
}

fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("opman-store-tests-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    root
}

#[test]
fn a_missing_directory_is_empty_rather_than_an_error() {
    let mut store = SkillStore::open(PathBuf::from("/nonexistent/opman/skills"));
    store.refresh();
    assert!(store.skills().is_empty());
}

#[test]
fn skills_are_read_from_disk() {
    let root = temp_root("read");
    write_skill(&root, "alpha", "does alpha", "BODY");
    let mut store = SkillStore::open(root.clone());
    store.refresh();
    let skill = store.get("alpha").expect("alpha loads");
    assert_eq!(skill.description, "does alpha");
    assert_eq!(skill.content, "BODY");
    let _ = std::fs::remove_dir_all(&root);
}

/// One malformed file must not take every other skill down with it, for every runner.
#[test]
fn a_malformed_skill_is_skipped_without_killing_the_scan() {
    let root = temp_root("malformed");
    write_skill(&root, "good", "fine", "BODY");
    let bad = root.join("bad");
    std::fs::create_dir_all(&bad).expect("create");
    std::fs::write(bad.join("SKILL.md"), "no frontmatter here").expect("write");
    let mut store = SkillStore::open(root.clone());
    store.refresh();
    assert!(store.get("good").is_some());
    assert!(store.get("bad").is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_directory_without_a_skill_file_is_ignored() {
    let root = temp_root("nofile");
    std::fs::create_dir_all(root.join("empty")).expect("create");
    let mut store = SkillStore::open(root.clone());
    store.refresh();
    assert!(store.skills().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// A skill added while a session is live must appear without restarting anything.
#[test]
fn a_stale_listing_is_rescanned() {
    let root = temp_root("rescan");
    write_skill(&root, "alpha", "a", "A");
    let mut store = SkillStore::open(root.clone());
    store.refresh();
    assert_eq!(store.skills().len(), 1);

    write_skill(&root, "beta", "b", "B");
    // Within the TTL nothing re-reads the directory.
    assert!(!store.refresh());
    assert_eq!(store.skills().len(), 1);

    std::thread::sleep(Duration::from_millis(300));
    // Past it, the new skill appears and the tool set is reported as changed.
    assert!(store.refresh());
    assert_eq!(store.skills().len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_edit_that_leaves_the_tool_set_alone_is_not_reported_as_changed() {
    let root = temp_root("edit");
    write_skill(&root, "alpha", "a", "A");
    let mut store = SkillStore::open(root.clone());
    store.refresh();

    write_skill(&root, "alpha", "a", "EDITED");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!store.refresh(), "same tools, so no listChanged");
    assert_eq!(store.get("alpha").expect("alpha").content, "EDITED");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn required_servers_are_collected_and_deduplicated() {
    let root = temp_root("requires");
    for (name, requires) in [("a", "jira"), ("b", "jira"), ("c", "linear")] {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).expect("create");
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\ndescription: d\nrequires: [{requires}]\n---\nbody\n"),
        )
        .expect("write");
    }
    let mut store = SkillStore::open(root.clone());
    store.refresh();
    let servers: Vec<_> = store.required_servers().into_iter().collect();
    assert_eq!(servers, ["jira", "linear"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_traversal_shaped_directory_name_is_ignored() {
    let root = temp_root("traversal");
    write_skill(&root, ".hidden", "d", "B");
    let mut store = SkillStore::open(root.clone());
    store.refresh();
    assert!(store.skills().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
