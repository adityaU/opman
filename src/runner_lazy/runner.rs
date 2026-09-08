use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};
use tokio::sync::{broadcast, OnceCell};

use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerSession, SlashCommand};

use super::{LazyContext, LazyStart};

type StartFuture = Pin<Box<dyn Future<Output = Result<LazyStart>> + Send>>;
type StartFn = Box<dyn Fn() -> StartFuture + Send + Sync>;

/// A runner slot that is served, but not yet running.
pub struct LazyRunner {
    kind: RunnerKind,
    /// Set only for ACP agents: the agent id this slot is reserved for. The supervisor
    /// reads it to tell "reserved for me, not started" apart from "held by someone else".
    acp_agent: Option<String>,
    ctx: Arc<LazyContext>,
    inner: OnceCell<Arc<dyn Runner>>,
    start: StartFn,
}

impl LazyRunner {
    pub fn new<F, Fut>(kind: RunnerKind, ctx: Arc<LazyContext>, start: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<LazyStart>> + Send + 'static,
    {
        Self {
            kind,
            acp_agent: None,
            ctx,
            inner: OnceCell::new(),
            start: Box::new(move || Box::pin(start())),
        }
    }

    /// Mark this slot as reserved for an ACP agent id.
    pub fn for_acp_agent(mut self, id: impl Into<String>) -> Self {
        self.acp_agent = Some(id.into());
        self
    }

    /// Whether the real runner is up.
    pub fn is_started(&self) -> bool {
        self.inner.get().is_some()
    }

    /// The started runner, starting it if this is the first use.
    async fn ensure(&self) -> Result<&Arc<dyn Runner>> {
        self.inner
            .get_or_try_init(|| async {
                let started = (self.start)().await?;
                let registry = self.ctx.registry();
                if let Some(registry) = &registry {
                    registry.install(self.kind.clone(), started.runner.clone());
                }
                if let Some(handle) = started.handle {
                    self.ctx.push_handle(handle);
                }
                if let (Some(supervisor), Some((id, engine))) =
                    (self.ctx.supervisor(), started.engine)
                {
                    supervisor.adopt_one(id, self.kind.clone(), engine).await;
                }
                if let Some(registry) = &registry {
                    registry.notify_started(&self.kind, &started.runner);
                }
                tracing::info!(runner = %self.kind.display_name(), "runner started on first use");
                Ok(started.runner)
            })
            .await
    }
}

/// Delegate to the started runner, or answer `$default` without starting one.
macro_rules! if_started {
    ($self:ident, $default:expr, |$runner:ident| $call:expr) => {
        match $self.inner.get() {
            Some($runner) => $call,
            None => Box::pin(async { Ok($default) }),
        }
    };
}

impl Runner for LazyRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }

    fn pending_acp_agent(&self) -> Option<String> {
        if self.is_started() {
            return None;
        }
        self.acp_agent.clone()
    }

    fn event_url(&self) -> Option<String> {
        self.inner.get().and_then(|runner| runner.event_url())
    }

    fn event_receiver(&self) -> Option<broadcast::Receiver<String>> {
        self.inner.get().and_then(|runner| runner.event_receiver())
    }

    fn create_session<'a>(
        &'a self,
        directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move { self.ensure().await?.create_session(directory, title).await })
    }

    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.ensure()
                .await?
                .send_message(session_id, directory, body)
                .await
        })
    }

    fn sessions<'a>(
        &'a self,
        directory: &'a str,
    ) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        if_started!(self, Vec::new(), |runner| runner.sessions(directory))
    }

    fn messages<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, Value> {
        if_started!(self, json!([]), |runner| runner
            .messages(session_id, directory))
    }

    fn status<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        if_started!(self, json!({}), |runner| runner.status(directory))
    }

    fn providers<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        if_started!(
            self,
            json!({ "all": [], "connected": [], "default": {} }),
            |runner| runner.providers(directory)
        )
    }

    fn agents<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        if_started!(self, json!([]), |runner| runner.agents(directory))
    }

    fn commands<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        if_started!(self, json!([]), |runner| runner.commands(directory))
    }

    fn configure<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        choices: &'a crate::app::EngineChoices,
    ) -> RunnerFuture<'a, bool> {
        if_started!(self, false, |runner| runner
            .configure(session_id, directory, choices))
    }

    fn execute_command<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        command: SlashCommand<'a>,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.ensure()
                .await?
                .execute_command(session_id, directory, command)
                .await
        })
    }

    fn abort<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, ()> {
        if_started!(self, (), |runner| runner.abort(session_id, directory))
    }

    fn rename<'a>(
        &'a self,
        session_id: &'a str,
        title: &'a str,
        directory: &'a str,
    ) -> RunnerFuture<'a, bool> {
        if_started!(self, false, |runner| runner
            .rename(session_id, title, directory))
    }

    fn delete<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, bool> {
        if_started!(self, false, |runner| runner.delete(session_id, directory))
    }

    fn reply_permission<'a>(
        &'a self,
        request_id: &'a str,
        reply: &'a str,
    ) -> RunnerFuture<'a, bool> {
        if_started!(self, false, |runner| runner
            .reply_permission(request_id, reply))
    }

    fn reply_question<'a>(
        &'a self,
        request_id: &'a str,
        answers: &'a [Vec<String>],
    ) -> RunnerFuture<'a, bool> {
        if_started!(self, false, |runner| runner
            .reply_question(request_id, answers))
    }

    fn reject_question<'a>(&'a self, request_id: &'a str) -> RunnerFuture<'a, bool> {
        if_started!(self, false, |runner| runner.reject_question(request_id))
    }
}
