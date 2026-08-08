//! The patch semantics the composer depends on.
//!
//! The endpoint itself is a thin route to the runner; what is worth pinning down is the
//! local mirror it keeps, because that is what the client reads back and the property it
//! must have — one chip's change leaves the other three alone — is easy to lose.

use crate::app::{EngineChoices, SessionInfo};
use crate::web::web_state::WebStateHandle;

async fn session_with(engine: EngineChoices) -> WebStateHandle {
    let state = WebStateHandle::new_test_with_projects(vec![(
        "proj".to_string(),
        std::path::PathBuf::from("/tmp/proj"),
    )]);
    state
        .add_and_activate_session(
            0,
            SessionInfo {
                id: "ses_1".to_string(),
                engine,
                ..SessionInfo::default()
            },
        )
        .await;
    state
}

fn choices(model: Option<&str>, effort: Option<&str>) -> EngineChoices {
    EngineChoices::from_parts(model, None, effort, None)
}

#[tokio::test]
async fn changing_one_chip_leaves_the_others_alone() {
    let state = session_with(EngineChoices::from_parts(
        Some("opus"),
        Some("plan"),
        Some("high"),
        Some("acceptEdits"),
    ))
    .await;

    state
        .apply_session_engine("ses_1", &choices(Some("sonnet"), None))
        .await;

    let web = state.get_state().await;
    let session = &web.projects[0].sessions[0];
    assert_eq!(session.engine.model.as_deref(), Some("sonnet"));
    // The three the composer did not send must survive — this is the bug the whole
    // change exists to fix, in miniature.
    assert_eq!(session.engine.agent.as_deref(), Some("plan"));
    assert_eq!(session.engine.effort.as_deref(), Some("high"));
    assert_eq!(
        session.engine.permission_mode.as_deref(),
        Some("acceptEdits")
    );
}

#[tokio::test]
async fn a_session_that_was_never_configured_takes_the_first_choice() {
    let state = session_with(EngineChoices::default()).await;

    state
        .apply_session_engine("ses_1", &choices(None, Some("low")))
        .await;

    let web = state.get_state().await;
    let engine = &web.projects[0].sessions[0].engine;
    assert_eq!(engine.effort.as_deref(), Some("low"));
    assert_eq!(engine.model, None);
}

#[tokio::test]
async fn an_unknown_session_is_ignored_rather_than_invented() {
    let state = session_with(EngineChoices::default()).await;

    state
        .apply_session_engine("ses_missing", &choices(Some("opus"), None))
        .await;

    let web = state.get_state().await;
    assert_eq!(web.projects[0].sessions.len(), 1);
    assert_eq!(web.projects[0].sessions[0].engine.model, None);
}
