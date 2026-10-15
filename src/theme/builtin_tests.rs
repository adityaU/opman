//! The built-in themes resolve by name, so a theme picked from the list always has colours.

use super::*;

#[test]
fn every_listed_theme_loads_by_name() {
    let names: Vec<String> = files()
        .filter_map(|file| Some(file.path().file_stem()?.to_string_lossy().into_owned()))
        .collect();
    assert!(!names.is_empty(), "opman ships themes");
    for name in &names {
        let json = theme(name).unwrap_or_else(|| panic!("{name} loads"));
        assert!(json.get("theme").is_some(), "{name} has a theme section");
    }
}

#[test]
fn an_unknown_theme_is_none() {
    assert!(theme("no-such-theme").is_none());
}
