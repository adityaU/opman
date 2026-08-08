use super::*;

use std::time::Duration;

use axum::http::HeaderValue;

use crate::app::{SessionInfo, SessionTime};
use crate::web::test_support::{test_server_state, test_server_state_with_projects};

fn headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(token) {
        headers.insert("x-internal-token", value);
    }
    headers
}

fn body(session: &str, directory: &str) -> AskRequest {
    AskRequest {
        session_id: session.to_string(),
        directory: directory.to_string(),
        questions: vec![json!({
            "question": "Which database?",
            "header": "DB",
            "options": [{ "label": "Postgres" }, { "label": "SQLite" }],
        })],
    }
}

/// A state whose project 0 is `/repo`, so seeded sessions have somewhere to live.
fn state_with_project() -> ServerState {
    test_server_state_with_projects(vec![("repo".into(), std::path::PathBuf::from("/repo"))])
}

type Events = tokio::sync::broadcast::Receiver<String>;

// ── authentication ──────────────────────────────────────────────────

#[tokio::test]
async fn a_wrong_or_missing_token_is_rejected() {
    let state = test_server_state();
    for token in ["", "not-the-token"] {
        let result = internal_ask(
            State(state.clone()),
            headers(token),
            Json(body("ses-1", "/repo")),
        )
        .await;
        assert!(
            matches!(result, Err(WebError::Unauthorized)),
            "token {token:?} should not be accepted"
        );
    }
}

#[tokio::test]
async fn a_request_with_nothing_to_ask_is_rejected() {
    let state = test_server_state();
    let empty = AskRequest {
        session_id: "ses-1".to_string(),
        directory: "/repo".to_string(),
        questions: Vec::new(),
    };
    let result = internal_ask(State(state), headers("test-internal-token"), Json(empty)).await;
    assert!(matches!(result, Err(WebError::BadRequest(_))));
}

// ── the round trip ──────────────────────────────────────────────────

#[tokio::test]
async fn an_answer_is_returned_to_the_asker_and_clears_the_card() {
    let state = test_server_state();
    let mut events = state.raw_sse_tx.subscribe();

    let asking = tokio::spawn({
        let state = state.clone();
        async move {
            internal_ask(
                State(state),
                headers("test-internal-token"),
                Json(body("ses-1", "/repo")),
            )
            .await
        }
    });

    let asked = wait_for(&mut events, "question.asked").await;
    assert_eq!(asked["properties"]["sessionID"], "ses-1");
    assert_eq!(asked["properties"]["questions"][0]["header"], "DB");
    let id = asked["properties"]["id"].as_str().expect("id").to_string();
    assert!(id.starts_with("qst_"), "got: {id}");

    assert!(state
        .ask_pending
        .resolve(&id, vec![vec!["Postgres".to_string()]])
        .is_ok());

    let response = asking
        .await
        .expect("handler completes")
        .expect("handler succeeds");
    assert_eq!(response.0["answers"][0][0], "Postgres");

    // The clearing event is what stops the card reappearing on the next reconnect.
    let cleared = wait_for(&mut events, "question.replied").await;
    assert_eq!(cleared["properties"]["requestID"], id.as_str());
}

#[tokio::test]
async fn a_dismissed_question_returns_no_answers() {
    let state = test_server_state();
    let mut events = state.raw_sse_tx.subscribe();
    let asking = tokio::spawn({
        let state = state.clone();
        async move {
            internal_ask(
                State(state),
                headers("test-internal-token"),
                Json(body("ses-1", "/repo")),
            )
            .await
        }
    });

    let id = asked_ref(&mut events).await;
    assert!(state.ask_pending.dismiss(&id));

    let response = asking
        .await
        .expect("handler completes")
        .expect("handler succeeds");
    let answers = response.0["answers"].as_array().expect("array");
    assert!(answers.is_empty(), "a dismissal is not an answer");
}

/// The asker hanging up — its turn was cancelled — must take the card down with it.
#[tokio::test]
async fn abandoning_the_request_retires_the_waiter() {
    let state = test_server_state();
    let mut events = state.raw_sse_tx.subscribe();
    let asking = tokio::spawn({
        let state = state.clone();
        async move {
            internal_ask(
                State(state),
                headers("test-internal-token"),
                Json(body("ses-1", "/repo")),
            )
            .await
        }
    });

    let id = asked_ref(&mut events).await;
    asking.abort();

    let cleared = wait_for(&mut events, "question.replied").await;
    assert_eq!(cleared["properties"]["id"], id.as_str());
    assert!(
        !state.ask_pending.dismiss(&id),
        "a late reply must not resolve an abandoned request"
    );
}

// ── session attribution ─────────────────────────────────────────────

#[tokio::test]
async fn without_a_session_id_the_newest_session_in_the_directory_is_used() {
    let state = state_with_project();
    let mut events = state.raw_sse_tx.subscribe();
    seed(&state, "ses-old", "/repo", 100).await;
    seed(&state, "ses-new", "/repo", 900).await;
    seed(&state, "ses-elsewhere", "/other", 5000).await;

    let asking = tokio::spawn({
        let state = state.clone();
        async move {
            internal_ask(
                State(state),
                headers("test-internal-token"),
                Json(body("", "/repo")),
            )
            .await
        }
    });

    let asked = wait_for(&mut events, "question.asked").await;
    assert_eq!(asked["properties"]["sessionID"], "ses-new");
    asking.abort();
}

#[tokio::test]
async fn an_unknown_directory_still_raises_the_card() {
    let state = test_server_state();
    let mut events = state.raw_sse_tx.subscribe();
    let asking = tokio::spawn({
        let state = state.clone();
        async move {
            internal_ask(
                State(state),
                headers("test-internal-token"),
                Json(body("", "/nowhere")),
            )
            .await
        }
    });

    // Better an unattributed question the user can still answer than a silent drop.
    let asked = wait_for(&mut events, "question.asked").await;
    assert_eq!(asked["properties"]["sessionID"], "");
    asking.abort();
}

// ── helpers ─────────────────────────────────────────────────────────

async fn seed(state: &ServerState, id: &str, directory: &str, updated: u64) {
    state
        .web_state
        .add_and_activate_session(
            0,
            SessionInfo {
                id: id.to_string(),
                directory: directory.to_string(),
                time: SessionTime {
                    created: updated,
                    updated,
                },
                ..Default::default()
            },
        )
        .await;
}

/// The id of the next question raised on this stream.
async fn asked_ref(events: &mut Events) -> String {
    wait_for(events, "question.asked").await["properties"]["id"]
        .as_str()
        .expect("every card carries an id")
        .to_string()
}

async fn wait_for(events: &mut Events, wanted: &str) -> Value {
    loop {
        let raw = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("an event arrives")
            .expect("channel open");
        let event: Value = serde_json::from_str(&raw).expect("json");
        if event["type"] == wanted {
            return event;
        }
    }
}
