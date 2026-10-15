//! Target permissions reach the handoff validator and persist on the new session.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::retarget;
use crate::app::EngineChoices;
use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerRegistry, RunnerSession};
use crate::runner_limits::settings::FallbackPair;

struct PermissionRunner {
    kind: RunnerKind,
    configured: Mutex<Option<String>>,
}

impl PermissionRunner {
    fn new(kind: RunnerKind) -> Self {
        Self {
            kind,
            configured: Mutex::new(None),
        }
    }
}

impl Runner for PermissionRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }

    fn create_session<'a>(&'a self, _: &'a str, title: &'a str) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            Ok(RunnerSession {
                id: "permission_handoff".into(),
                title: title.into(),
            })
        })
    }

    fn messages<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!([])) })
    }

    fn send_message<'a>(&'a self, _: &'a str, _: &'a str, body: Value) -> RunnerFuture<'a, Value> {
        Box::pin(async move { Ok(body) })
    }

    fn abort<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn providers<'a>(&'a self, _: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async {
            Ok(json!({"permissionModes": [{"value": "read-only", "label": "Read only"}]}))
        })
    }

    fn configure<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        choices: &'a EngineChoices,
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            *self.configured.lock().await = choices
                .permission_mode
                .as_ref()
                .map(|mode| mode.as_str().into());
            Ok(true)
        })
    }
}

#[tokio::test]
async fn fallback_permission_is_validated_and_recorded_on_the_target() -> anyhow::Result<()> {
    for (permission, expected) in [("read-only", Some("read-only")), ("unsupported", None)] {
        let source = Arc::new(PermissionRunner::new(RunnerKind::Opencode));
        let target = Arc::new(PermissionRunner::new(RunnerKind::ClaudeCode));
        let runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::from([
            (RunnerKind::Opencode, source as Arc<dyn Runner>),
            (RunnerKind::ClaudeCode, target.clone() as Arc<dyn Runner>),
        ]);
        let registry = RunnerRegistry::new(RunnerKind::Opencode, runners);
        let pair = FallbackPair {
            runner: "claude-code".into(),
            model: None,
            permission: Some(permission.into()),
        };
        let mut body = json!({"parts": [], "permission": "bypassPermissions"});
        retarget(&mut body, &pair);
        let outcome = registry
            .send_message(
                "source_permission",
                "/project",
                Some(RunnerKind::ClaudeCode),
                body,
            )
            .await?;
        assert!(outcome.switched);
        assert_eq!(
            outcome.response.get("permission").and_then(Value::as_str),
            expected
        );
        assert_eq!(target.configured.lock().await.as_deref(), expected);
    }
    Ok(())
}
