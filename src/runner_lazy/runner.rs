use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};
use tokio::sync::{broadcast, OnceCell};

use crate::runner::{
    Runner, RunnerFuture, RunnerKind, RunnerSession, RunnerStartOutcome, SlashCommand,
};

use super::disk::DiskSessions;
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
    /// Where the runner keeps its sessions, so they can be listed before it starts.
    disk: Option<DiskSessions>,
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
            disk: None,
        }
    }

    /// List sessions from `store` while the runner is not started.
    pub fn with_disk_sessions(mut self, store: Option<crate::session_disk::SessionStore>) -> Self {
        self.disk = store.map(DiskSessions::new);
        self
    }

    /// Whether `session_id` was listed from disk, so acting on it needs the real runner.
    fn listed_on_disk(&self, session_id: &str) -> bool {
        self.disk
            .as_ref()
            .is_some_and(|disk| disk.listed(session_id))
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
                let handle = started.handle;
                if let Some(handle) = &handle {
                    self.ctx.push_handle(Arc::clone(handle));
                }
                if let Err(error) = started.runner.wait_until_ready().await {
                    if let Some(handle) = &handle {
                        crate::server::kill_server(handle);
                    }
                    return Err(error);
                }
                if let Some(url) = started.base_url {
                    crate::app::init_base_url(url);
                }
                let registry = self.ctx.registry();
                if let Some(registry) = &registry {
                    registry.install(self.kind.clone(), started.runner.clone());
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

/// Delegate to the started runner. A session listed from disk starts it — that session is
/// real and only its runner can render or change it; anything else answers `$default`.
macro_rules! if_started_or_listed {
    ($self:ident, $session:expr, $default:expr, |$runner:ident| $call:expr) => {
        Box::pin(async move {
            let $runner = match $self.inner.get() {
                Some(runner) => runner,
                None if $self.listed_on_disk($session) => $self.ensure().await?,
                None => return Ok($default),
            };
            $call.await
        })
    };
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

    fn ensure_started<'a>(&'a self) -> RunnerFuture<'a, RunnerStartOutcome> {
        Box::pin(async move {
            let was_started = self.is_started();
            self.ensure().await?;
            Ok(if was_started {
                RunnerStartOutcome::AlreadyStarted
            } else {
                RunnerStartOutcome::Started
            })
        })
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
        match (self.inner.get(), &self.disk) {
            (Some(runner), _) => runner.sessions(directory),
            (None, Some(disk)) => Box::pin(async move { Ok(disk.list(directory).await) }),
            (None, None) => Box::pin(async { Ok(Vec::new()) }),
        }
    }

    fn messages<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, Value> {
        if_started_or_listed!(self, session_id, json!([]), |runner| runner
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
        if_started_or_listed!(self, session_id, false, |runner| runner
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
        if_started_or_listed!(self, session_id, false, |runner| runner
            .rename(session_id, title, directory))
    }

    fn delete<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, bool> {
        if_started_or_listed!(self, session_id, false, |runner| runner
            .delete(session_id, directory))
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
