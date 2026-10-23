//! Publish a completed handoff to the sidebar and every open chat pane.
use crate::runner::SendOutcome;
use crate::web::types::{ServerState, WebEvent};

pub(super) async fn publish(
    state: &ServerState,
    session_id: &str,
    dir: &str,
    outcome: &SendOutcome,
) {
    // A handoff creates a runner-native session. Add it to the same
    // project immediately so the sidebar can render the new runner before
    // the normal session poller sees it.
    if !outcome.switched {
        return;
    }
    let project_idx = state.web_state.active_project_index().await;
    let now = chrono::Utc::now().timestamp_millis() as u64;
    state
        .web_state
        .add_and_activate_session(
            project_idx,
            crate::app::SessionInfo {
                id: outcome.session_id.clone(),
                title: "Handoff session".to_string(),
                directory: dir.to_string(),
                time: crate::app::SessionTime {
                    created: now,
                    updated: now,
                },
                ..Default::default()
            },
        )
        .await;
    state
        .web_state
        .set_session_runner(&outcome.session_id, &outcome.runner.display_name())
        .await;
    // The handoff prompt was this session's opening turn.
    state
        .web_state
        .mark_instructions_delivered(&outcome.session_id)
        .await;
    // The turn moved to the new session; the one being left is done.
    state.web_state.mark_turn_settled(session_id).await;
    state
        .web_state
        .mark_turn_dispatched(&outcome.session_id)
        .await;
    let _ = state.event_tx.send(WebEvent::SessionHandoff {
        from: session_id.to_string(),
        to: outcome.session_id.clone(),
        runner: outcome.runner.display_name().into_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::RunnerKind;

    #[tokio::test]
    async fn manual_handoff_publishes_target_and_runner_for_all_panes() -> anyhow::Result<()> {
        let state = crate::web::test_support::test_server_state();
        let mut events = state.event_tx.subscribe();
        let outcome = SendOutcome {
            session_id: "manual_target".into(),
            runner: RunnerKind::ClaudeCode,
            switched: true,
            response: serde_json::json!({"ok": true}),
        };
        publish(&state, "manual_source", "/project", &outcome).await;
        let mut found = false;
        while let Ok(event) = events.try_recv() {
            if let WebEvent::SessionHandoff { from, to, runner } = event {
                assert_eq!(from, "manual_source");
                assert_eq!(to, "manual_target");
                assert_eq!(runner, "claude-code");
                found = true;
            }
        }
        assert!(found);
        assert_eq!(
            state
                .web_state
                .session_runner("manual_target")
                .await
                .as_deref(),
            Some("claude-code")
        );
        assert!(
            state
                .web_state
                .instructions_delivered("manual_target")
                .await
        );
        Ok(())
    }

    #[tokio::test]
    async fn ordinary_send_does_not_publish_a_handoff() {
        let state = crate::web::test_support::test_server_state();
        let mut events = state.event_tx.subscribe();
        let outcome = SendOutcome {
            session_id: "same_session".into(),
            runner: RunnerKind::Opencode,
            switched: false,
            response: serde_json::Value::Null,
        };
        publish(&state, "same_session", "/project", &outcome).await;
        assert!(events.try_recv().is_err());
    }
}
