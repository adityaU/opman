use super::*;
use std::fs;
use tempfile::TempDir;

const REL_SERVER: &str = "server-code/packages/opencode/src/cli/cmd/tui/context/theme";
const REL_PACKAGES: &str = "packages/opencode/src/cli/cmd/tui/context/theme";

#[test]
fn find_themes_dir_server_code_variant() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join(REL_SERVER);
    fs::create_dir_all(&target).unwrap();
    assert_eq!(find_opencode_themes_dir(tmp.path()), Some(target));
}

#[test]
fn find_themes_dir_packages_variant() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join(REL_PACKAGES);
    fs::create_dir_all(&target).unwrap();
    assert_eq!(find_opencode_themes_dir(tmp.path()), Some(target));
}

#[test]
fn find_themes_dir_walks_up_to_ancestor() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join(REL_SERVER);
    fs::create_dir_all(&target).unwrap();
    // Start from a deep child; the fn should climb to the ancestor.
    let deep = tmp.path().join("a/b/c");
    fs::create_dir_all(&deep).unwrap();
    assert_eq!(find_opencode_themes_dir(&deep), Some(target));
}

#[test]
fn find_themes_dir_none_when_absent() {
    let tmp = TempDir::new().unwrap();
    let deep = tmp.path().join("x/y/z");
    fs::create_dir_all(&deep).unwrap();
    // No opencode theme tree along the path -> None (walks up to filesystem root).
    assert_eq!(find_opencode_themes_dir(&deep), None);
}
