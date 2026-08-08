//! Where a question reply goes.
//!
//! There are two kinds of question in opman now — one an engine raised over its own
//! protocol, one the `ask` MCP server raised over the loopback API — and the reply routes
//! have to tell them apart from nothing but a request id. These assert the order: the
//! local registry first, the runner fan-out only for an id it does not hold.

use super::*;
use crate::web::test_support::{test_router, test_server_state};
use axum::http::StatusCode;
use serde_json::json;

fn answers() -> Vec<Vec<String>> {
    vec![vec!["Postgres".to_string()]]
}

fn auth() -> AuthUser {
    AuthUser {
        subject: "test".to_string(),
    }
}

/// A locally-held question is answered here, without touching the network. The upstream
/// base url is deliberately left unset: if the handler fell through to the fan-out, the
/// request would fail rather than quietly succeed against a real server.
#[tokio::test]
async fn a_reply_to_a_locally_held_question_never_reaches_a_runner() {
    let state = test_server_state();
    let waiting = state.ask_pending.register("qst_local", "ses-1");

    let response = reply_question(
        State(state.clone()),
        auth(),
        axum::extract::Path("qst_local".to_string()),
        Json(QuestionReplyRequest { answers: answers() }),
    )
    .await
    .expect("the local registry answers it");

    assert_eq!(response.into_response().status(), StatusCode::OK);
    assert_eq!(waiting.await.expect("answered"), answers());
}

#[tokio::test]
async fn dismissing_a_locally_held_question_closes_it_unanswered() {
    let state = test_server_state();
    let waiting = state.ask_pending.register("qst_local", "ses-1");

    let response = reject_question(
        State(state.clone()),
        auth(),
        axum::extract::Path("qst_local".to_string()),
    )
    .await
    .expect("the local registry dismisses it");

    assert_eq!(response.into_response().status(), StatusCode::OK);
    assert!(waiting.await.is_err(), "a dismissal is not an answer");
    assert!(!state.ask_pending.dismiss("qst_local"), "already retired");
}

/// A card the user dismisses after its asker gave up still has to disappear, so an id
/// nobody owns is a success rather than a 404.
#[tokio::test]
async fn dismissing_an_unowned_question_still_succeeds() {
    let state = test_server_state();
    let response = reject_question(
        State(state),
        auth(),
        axum::extract::Path("qst_nobody_owns".to_string()),
    )
    .await
    .expect("an unowned dismissal is not an error");
    assert_eq!(response.into_response().status(), StatusCode::OK);
}

/// Aborting a turn takes its questions down with it. The asker is being killed alongside
/// the turn, so a card left up would collect an answer nothing could receive.
#[tokio::test]
async fn aborting_a_session_clears_the_questions_it_raised() {
    let state = test_server_state();
    let mut events = state.raw_sse_tx.subscribe();
    let mine = state.ask_pending.register("qst_mine", "ses-1");
    let theirs = state.ask_pending.register("qst_theirs", "ses-2");

    clear_asked_questions(&state, "ses-1", "/repo").await;

    assert!(mine.await.is_err(), "the aborted session's card is retired");
    let cleared: serde_json::Value =
        serde_json::from_str(&events.recv().await.expect("an event")).expect("json");
    assert_eq!(cleared["type"], "question.rejected");
    assert_eq!(cleared["properties"]["requestID"], "qst_mine");

    // Another session's question is none of this abort's business.
    assert!(state.ask_pending.resolve("qst_theirs", answers()).is_ok());
    assert_eq!(theirs.await.expect("answered"), answers());
}

/// The route the frontend has always called, and which nothing served until now.
#[tokio::test]
async fn the_reject_route_is_wired_into_the_router() {
    let state = test_server_state();
    let waiting = state.ask_pending.register("qst_local", "ses-1");
    let router = test_router(state);

    let (status, _body) = crate::web::test_support::send_json(
        router,
        "POST",
        "/api/question/qst_local/reject",
        Some(json!({})),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(waiting.await.is_err());
}
