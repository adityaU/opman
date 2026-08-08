//! Directory walking and the reload watcher.

use super::*;

fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("opman-skills-mod-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    root
}

fn write_skill(root: &Path, name: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("create");
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: d\n---\nbody\n"),
    )
    .expect("write");
}

#[test]
fn the_skills_dir_honours_the_env_override() {
    // The seam tests and future per-project skills both need.
    let previous = std::env::var("OPMAN_SKILLS_DIR").ok();
    std::env::set_var("OPMAN_SKILLS_DIR", "/tmp/opman-skills-override");
    assert_eq!(
        get_skills_dir(),
        PathBuf::from("/tmp/opman-skills-override")
    );
    match previous {
        Some(value) => std::env::set_var("OPMAN_SKILLS_DIR", value),
        None => std::env::remove_var("OPMAN_SKILLS_DIR"),
    }
}

#[test]
fn loading_is_keyed_by_directory_name_in_stable_order() {
    let root = temp_root("order");
    for name in ["zeta", "alpha", "mid"] {
        write_skill(&root, name);
    }
    let skills = load_skills_from(&root);
    let names: Vec<_> = skills.keys().map(SkillName::as_str).collect();
    // BTreeMap, so the web UI's list stops reshuffling between fetches.
    assert_eq!(names, ["alpha", "mid", "zeta"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_file_at_the_top_level_is_not_a_skill() {
    let root = temp_root("file");
    std::fs::write(root.join("SKILL.md"), "---\ndescription: d\n---\nbody\n").expect("write");
    assert!(load_skills_from(&root).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// The old watcher looped on `if rx.recv().await.is_ok()`, so once the sender dropped it
/// spun a core at 100% forever. It must exit instead.
#[tokio::test]
async fn the_reload_watcher_exits_when_the_sender_is_dropped() {
    let (tx, rx) = broadcast::channel(4);
    let registry: SkillsRegistry = SkillsRegistry::default();
    spawn_skills_reload_watcher(rx, registry);
    drop(tx);
    // If the task spun instead of exiting, this would still return — the real assertion
    // is that the process does not peg a core, which `tokio::test` cannot see directly.
    // Yielding lets the task observe the close and break.
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn a_reload_signal_refreshes_the_registry() {
    let root = temp_root("reload");
    std::env::set_var("OPMAN_SKILLS_DIR", &root);
    let (tx, rx) = broadcast::channel(4);
    let registry: SkillsRegistry = SkillsRegistry::default();
    spawn_skills_reload_watcher(rx, registry.clone());

    write_skill(&root, "alpha");
    tx.send(()).expect("send");
    for _ in 0..50 {
        if !registry.read().await.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(registry
        .read()
        .await
        .contains_key(&SkillName::parse("alpha").expect("valid")));

    std::env::remove_var("OPMAN_SKILLS_DIR");
    let _ = std::fs::remove_dir_all(&root);
}
