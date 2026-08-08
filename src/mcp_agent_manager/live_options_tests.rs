//! `agent_runner_options` against the live catalogues.
//!
//! Split from [`super::live`] on size alone; the gating and the socket client are the same.

use serde_json::json;

use super::live_support::{call, directory, enabled, CLAUDE_MODEL};

/// The ceiling the reply has to stay under to be readable at all.
///
/// What it replaced was 10,966,000 bytes: every model of every provider opencode has ever
/// heard of, plus the raw tree, none of which fits in a context window.
const MAX_OPTIONS_BYTES: usize = 256 * 1024;

fn model_ids(options: &serde_json::Value) -> Vec<&str> {
    options["models"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model["id"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

/// The defect this exists for: the opencode reply was 11 MB, and no agent could read it to
/// find out what to pass to the two tools that require a model and an effort.
#[tokio::test]
#[ignore = "needs a running opman"]
async fn runner_options_fits_in_a_context_window_and_names_real_efforts() {
    if !enabled() {
        return;
    }
    let options =
        call(json!({ "op": "options", "directory": directory(), "runner": "opencode" })).await;

    let size = options.to_string().len();
    assert!(
        size < MAX_OPTIONS_BYTES,
        "agent_runner_options returned {size} bytes; it must stay readable",
    );
    let models = options["models"].as_array().cloned().unwrap_or_default();
    assert!(!models.is_empty(), "a connected runner must list models");
    assert!(
        models
            .iter()
            .any(|model| model["efforts"].as_array().is_some_and(|e| !e.is_empty())),
        "every live model reported zero efforts, which is the bug this replaced",
    );
    assert!(
        !options["efforts"]
            .as_array()
            .is_none_or(|efforts| efforts.is_empty()),
        "the union of efforts must not be empty: {}",
        options["efforts"],
    );
    assert!(options["total_models"].as_u64().unwrap_or(0) >= models.len() as u64);
}

/// Narrowing must never make a model unreachable — that is what makes capping the default
/// list safe.
#[tokio::test]
#[ignore = "needs a running opman"]
async fn a_filter_finds_a_model_the_default_listing_leaves_out() {
    if !enabled() {
        return;
    }
    let filtered = call(json!({
        "op": "options", "directory": directory(), "runner": "opencode", "filter": "luna",
    }))
    .await;

    let ids = model_ids(&filtered);
    assert!(ids.iter().any(|id| id.contains("luna")), "{ids:?}");
}

#[tokio::test]
#[ignore = "needs a running opman"]
async fn claude_lists_haiku_with_the_runner_that_serves_it() {
    if !enabled() {
        return;
    }
    let options =
        call(json!({ "op": "options", "directory": directory(), "runner": "claude" })).await;

    assert_eq!(options["runner"], "claude");
    let ids = model_ids(&options);
    assert!(ids.contains(&CLAUDE_MODEL), "{ids:?}");
}
