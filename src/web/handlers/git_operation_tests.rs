//! Refusals from the operation controls.

use super::git_integrate_tests::*;
use super::super::git_integrate::*;
use axum::extract::State;
use axum::response::Json;

#[tokio::test]
async fn skip_is_refused_during_a_merge() {
    let td = diverged_repo();
    let state = state_for(td.path());
    merge(&state, "side", false).await;

    let body = act(&state, GitOperationAction::Skip).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["failure"], "failed");
    assert!(
        body["message"]
            .as_str()
            .expect("message")
            .contains("merge cannot be skipped"),
        "{body}"
    );
    // The merge is still in flight — a refusal must not touch the repository.
    assert_eq!(status_of(&state, "").await["kind"], "merge");
}

#[tokio::test]
async fn action_with_nothing_in_flight_is_refused() {
    let td = init_repo();
    commit(td.path(), "a.txt", "a\n", "init");
    let state = state_for(td.path());

    for action in [
        GitOperationAction::Continue,
        GitOperationAction::Abort,
        GitOperationAction::Skip,
    ] {
        let body = act(&state, action).await;
        assert_eq!(body["ok"], false, "{body}");
        assert!(
            body["message"]
                .as_str()
                .expect("message")
                .contains("No merge, rebase"),
            "{body}"
        );
    }
}
