//! Detection tests. Root detection is the one that matters most: the wrong
//! root means the wrong crate graph, and rust-analyzer reports confidently
//! about a project the user is not editing.

use super::*;

#[test]
fn maps_known_extensions() {
    assert_eq!(language_for(Path::new("a/b/main.rs")), Some("rust"));
    assert_eq!(language_for(Path::new("x.tsx")), Some("typescriptreact"));
    assert_eq!(language_for(Path::new("X.PY")), Some("python"));
}

#[test]
fn unknown_extensions_have_no_language() {
    assert_eq!(language_for(Path::new("notes.unknownext")), None);
    assert_eq!(language_for(Path::new("LICENSE")), None);
}

/// Several languages deliberately share one server process.
#[test]
fn related_languages_share_a_server() {
    let ts = spec_for("typescript").unwrap();
    assert_eq!(spec_for("typescriptreact").unwrap().command, ts.command);
    assert_eq!(spec_for("javascript").unwrap().command, ts.command);
    assert_eq!(spec_for("cpp").unwrap().command, spec_for("c").unwrap().command);
}

#[test]
fn rust_maps_to_rust_analyzer() {
    let spec = spec_for("rust").unwrap();
    assert_eq!(spec.command, "rust-analyzer");
    assert!(spec.roots.contains(&"Cargo.toml"));
}

// ── Roots ───────────────────────────────────────────────

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "").unwrap();
}

#[test]
fn root_is_the_nearest_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    touch(&base.join("Cargo.toml"));
    touch(&base.join("crates/inner/Cargo.toml"));
    let file = base.join("crates/inner/src/lib.rs");
    touch(&file);

    let root = project_root(&file, base, &["Cargo.toml"]);
    assert_eq!(root, base.join("crates/inner"));
}

/// Every file in one crate must resolve to the same root, or each directory
/// would start its own copy of a gigabyte-scale process.
#[test]
fn sibling_files_share_a_root() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    touch(&base.join("Cargo.toml"));
    let a = base.join("src/one.rs");
    let b = base.join("src/deep/two.rs");
    touch(&a);
    touch(&b);

    assert_eq!(
        project_root(&a, base, &["Cargo.toml"]),
        project_root(&b, base, &["Cargo.toml"])
    );
}

#[test]
fn falls_back_to_git_when_no_marker_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    std::fs::create_dir_all(base.join(".git")).unwrap();
    let file = base.join("scripts/run.sh");
    touch(&file);

    assert_eq!(project_root(&file, base, &[]), base);
}

#[test]
fn falls_back_to_the_project_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    let file = base.join("a/b/c.rs");
    touch(&file);

    assert_eq!(project_root(&file, base, &["Cargo.toml"]), base);
}

/// The walk must not escape the project, or a stray Cargo.toml in a parent
/// directory would root the server outside the workspace entirely.
#[test]
fn never_climbs_above_the_project_dir() {
    let tmp = tempfile::tempdir().unwrap();
    touch(&tmp.path().join("Cargo.toml"));
    let project = tmp.path().join("nested");
    let file = project.join("src/main.rs");
    touch(&file);

    let root = project_root(&file, &project, &["Cargo.toml"]);
    assert!(root.starts_with(&project), "resolved outside project: {root:?}");
}

/// A missing binary is a normal state, not a failure — it decides whether the
/// language shows up as available at all.
#[test]
fn missing_binaries_resolve_to_none() {
    assert!(resolve_binary("definitely-not-a-real-language-server").is_none());
}
