use std::collections::HashSet;

use super::*;

#[test]
fn sanitizes_server_names() {
    let none = HashSet::new();
    assert_eq!(
        Namespace::for_server("opman-tools", &none).as_str(),
        "opman_tools"
    );
    assert_eq!(Namespace::for_server("9lives", &none).as_str(), "_9lives");
    assert_eq!(Namespace::for_server("", &none).as_str(), "server");
    assert_eq!(Namespace::for_server("a b.c", &none).as_str(), "a_b_c");
}

#[test]
fn collisions_get_a_suffix() {
    let mut taken = HashSet::new();
    let first = Namespace::for_server("a-b", &taken);
    taken.insert(first.clone());
    let second = Namespace::for_server("a_b", &taken);
    assert_eq!(first.as_str(), "a_b");
    assert_eq!(second.as_str(), "a_b_2");
}

#[test]
fn the_reserved_namespace_is_never_handed_out() {
    let ns = Namespace::for_server("$codemode", &HashSet::new());
    assert_ne!(ns.as_str(), CODEMODE);
}

#[test]
fn identifiers() {
    assert!(is_identifier("abc_1"));
    assert!(is_identifier("$x"));
    assert!(!is_identifier("1a"));
    assert!(!is_identifier("a-b"));
    assert!(!is_identifier(""));
}

#[test]
fn access_paths() {
    assert_eq!(access_path("time", "now"), "tools.time.now");
    assert_eq!(access_path("time", "get-now"), "tools.time[\"get-now\"]");
}

#[test]
fn suggestions_rank_close_names() {
    let names = ["read_file", "write_file", "grep", "glob"];
    assert_eq!(closest("read_fil", names.into_iter()), ["read_file"]);
    assert_eq!(closest("greep", names.into_iter()), ["grep"]);
    assert!(closest("zzzzzzzz", names.into_iter()).is_empty());
    assert!(closest("file", names.into_iter()).contains(&"read_file"));
}
