use std::sync::Arc;

use serde_json::{json, Value};

use crate::app::SessionInfo;
use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerRegistry, RunnerSession};

struct SessionRunner {
    sessions: Vec<SessionInfo>,
}

impl Runner for SessionRunner {
    fn kind(&self) -> RunnerKind {
        RunnerKind::Claude
    }

    fn create_session<'a>(
        &'a self,
        _directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            Ok(RunnerSession {
                id: "created".to_string(),
                title: title.to_string(),
            })
        })
    }

    fn sessions<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Vec<SessionInfo>> {
        Box::pin(async move { Ok(self.sessions.clone()) })
    }

    fn messages<'a>(&'a self, _id: &'a str, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!([])) })
    }

    fn send_message<'a>(
        &'a self,
        _id: &'a str,
        _directory: &'a str,
        _body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({})) })
    }

    fn abort<'a>(&'a self, _id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn runner_start_refreshes_sessions_without_waiting_for_poll() {
    let directory = "/project";
    let mut web_state = super::WebStateHandle::new_test_with_projects(vec![(
        "project".to_string(),
        directory.into(),
    )]);
    let registry = Arc::new(RunnerRegistry::new(
        RunnerKind::Opencode,
        std::collections::HashMap::new(),
    ));
    web_state.attach_runner_registry_for_test(registry.clone());

    let runner: Arc<dyn Runner> = Arc::new(SessionRunner {
        sessions: vec![crate::web::test_support::test_session(
            "claude-session",
            "",
            directory,
            3,
        )],
    });
    registry.notify_started(&RunnerKind::Claude, &runner);

    for _ in 0..16 {
        if web_state
            .get_project_sessions(0)
            .await
            .is_some_and(|(_, _, sessions)| sessions.iter().any(|(id, _)| id == "claude-session"))
        {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("runner startup did not refresh the project sessions");
}
