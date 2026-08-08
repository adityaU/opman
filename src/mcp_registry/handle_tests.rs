//! Swapping the registry underneath live readers.

use super::*;
use crate::mcp_registry::spec::{Arg, ServerSpec};

fn registry(names: &[&str]) -> Arc<McpRegistry> {
    Arc::new(McpRegistry::from_specs(
        names
            .iter()
            .map(|n| ServerSpec::stdio(*n, "/opman", vec![Arg::lit("x")], Vec::new()))
            .collect(),
        BuiltinFlags::default(),
    ))
}

fn names(registry: &McpRegistry) -> Vec<String> {
    registry
        .for_runner(&opman_backend_contracts::RunnerKind::Opencode)
        .map(|s| s.name().to_string())
        .collect()
}

#[test]
fn current_returns_what_was_installed() {
    let handle = RegistryHandle::new(registry(&["a"]), BuiltinFlags::default());
    assert_eq!(names(&handle.current()), ["a"]);
}

#[test]
fn replacing_is_visible_to_the_next_read() {
    let handle = RegistryHandle::new(registry(&["a"]), BuiltinFlags::default());
    handle.replace(registry(&["b", "c"]));
    assert_eq!(names(&handle.current()), ["b", "c"]);
}

/// A reader that already took a snapshot keeps working against it, so a swap can never
/// tear a payload that is half-built.
#[test]
fn an_existing_snapshot_is_unaffected_by_a_swap() {
    let handle = RegistryHandle::new(registry(&["a"]), BuiltinFlags::default());
    let snapshot = handle.current();
    handle.replace(registry(&["b"]));
    assert_eq!(names(&snapshot), ["a"]);
    assert_eq!(names(&handle.current()), ["b"]);
}

#[test]
fn a_handle_clone_shares_the_same_slot() {
    let handle = RegistryHandle::new(registry(&["a"]), BuiltinFlags::default());
    let other = handle.clone();
    other.replace(registry(&["b"]));
    assert_eq!(names(&handle.current()), ["b"]);
}

/// A panic in one reader must not take every runner's MCP configuration down with it.
#[test]
fn a_poisoned_lock_still_serves_reads() {
    let handle = RegistryHandle::new(registry(&["a"]), BuiltinFlags::default());
    let poisoner = handle.clone();
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.inner.write().expect("write lock");
        panic!("poison the lock");
    })
    .join();
    assert_eq!(names(&handle.current()), ["a"]);
    handle.replace(registry(&["b"]));
    assert_eq!(names(&handle.current()), ["b"]);
}

#[test]
fn flags_are_carried_for_reloads() {
    let handle = RegistryHandle::new(registry(&[]), BuiltinFlags::ALL);
    assert_eq!(handle.flags(), BuiltinFlags::ALL);
}
