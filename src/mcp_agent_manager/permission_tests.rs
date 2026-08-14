//! Resolving and inheriting a permission mode.

use super::*;
use crate::mcp_agent_manager::fake_runner::{Harness, DIR};

#[tokio::test]
async fn a_mode_the_runner_publishes_is_accepted() {
    let harness = Harness::new();

    let mode = PermissionMode::resolve(
        &harness.state.registry,
        RunnerKind::Claude,
        DIR,
        "bypassPermissions",
    )
    .await
    .expect("claude offers it");

    assert_eq!(mode.as_str(), "bypassPermissions");
}

/// The names belong to one runner's vocabulary, and a mode the target has never heard of
/// is stored, pushed and silently ignored — the failure this check exists to prevent.
#[tokio::test]
async fn a_mode_from_another_runner_is_refused_with_the_real_list() {
    let harness = Harness::new();

    let error = PermissionMode::resolve(
        &harness.state.registry,
        RunnerKind::Opencode,
        DIR,
        "bypassPermissions",
    )
    .await
    .expect_err("opencode offers no such mode");

    let error = format!("{error}");
    assert!(error.contains("bypassPermissions"), "{error}");
    assert!(error.contains("build, plan"), "{error}");
}

#[tokio::test]
async fn an_empty_mode_is_refused_rather_than_stored_as_a_choice_of_nothing() {
    let harness = Harness::new();

    let error = PermissionMode::resolve(&harness.state.registry, RunnerKind::Claude, DIR, "  ")
        .await
        .expect_err("empty is not a mode");

    assert!(
        format!("{error}").contains("'permission' was empty"),
        "{error}"
    );
}

/// A child on the caller's own runner starts where the caller is, so an unattended fleet
/// does not stop on prompts nobody is there to answer.
#[tokio::test]
async fn a_child_on_the_same_runner_inherits_the_callers_mode() {
    let harness = Harness::new();
    let parent = harness
        .session(RunnerKind::Claude, "bypassPermissions")
        .await;

    let inherited =
        PermissionMode::inherited(&harness.state.registry, &RunnerKind::Claude, DIR, &parent).await;

    assert_eq!(
        inherited.map(|mode| mode.as_str().to_string()).as_deref(),
        Some("bypassPermissions")
    );
}

#[tokio::test]
async fn nothing_is_inherited_across_runners_or_from_no_caller_at_all() {
    let harness = Harness::new();
    let parent = harness
        .session(RunnerKind::Claude, "bypassPermissions")
        .await;

    let across =
        PermissionMode::inherited(&harness.state.registry, &RunnerKind::Opencode, DIR, &parent)
            .await;
    assert_eq!(across, None, "a claude mode means nothing to opencode");

    let orphan =
        PermissionMode::inherited(&harness.state.registry, &RunnerKind::Claude, DIR, "").await;
    assert_eq!(orphan, None);

    let unconfigured = harness.session(RunnerKind::Claude, "").await;
    let none = PermissionMode::inherited(
        &harness.state.registry,
        &RunnerKind::Claude,
        DIR,
        &unconfigured,
    )
    .await;
    assert_eq!(none, None, "a session with no mode has none to pass on");
}
