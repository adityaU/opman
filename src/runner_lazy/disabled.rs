//! A runner slot that is switched off in settings.

use serde_json::Value;

use crate::runner::{
    Runner, RunnerDisabled, RunnerFuture, RunnerKind, RunnerSession, RunnerStartOutcome,
    SlashCommand,
};

use super::disk::DiskSessions;

/// Stands in the registry where a switched-off runner would be.
///
/// The sessions already in it stay listed, read from the runner's store without starting
/// anything: turning a runner off is a choice about what runs, and history vanishing from
/// the sidebar would read as data loss. Everything else is refused with [`RunnerDisabled`]
/// — this type has no way to start a server, so a disabled runner cannot be booted by any
/// path that reaches it.
pub struct DisabledRunner {
    kind: RunnerKind,
    disk: Option<DiskSessions>,
}

impl DisabledRunner {
    pub fn new(kind: RunnerKind, store: Option<crate::session_disk::SessionStore>) -> Self {
        Self {
            kind,
            disk: store.map(DiskSessions::new),
        }
    }

    fn refuse<'a, T: Send + 'a>(&self) -> RunnerFuture<'a, T> {
        let error = RunnerDisabled::new(self.kind.clone());
        Box::pin(async move { Err(error.into()) })
    }
}

impl Runner for DisabledRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }

    fn is_disabled(&self) -> bool {
        true
    }

    fn ensure_started<'a>(&'a self) -> RunnerFuture<'a, RunnerStartOutcome> {
        self.refuse()
    }

    fn create_session<'a>(
        &'a self,
        _directory: &'a str,
        _title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        self.refuse()
    }

    fn sessions<'a>(
        &'a self,
        directory: &'a str,
    ) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        Box::pin(async move {
            Ok(match &self.disk {
                Some(disk) => disk.list(directory).await,
                None => Vec::new(),
            })
        })
    }

    fn messages<'a>(&'a self, _session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, Value> {
        self.refuse()
    }

    fn providers<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        self.refuse()
    }

    fn agents<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        self.refuse()
    }

    fn commands<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        self.refuse()
    }

    fn send_message<'a>(
        &'a self,
        _session_id: &'a str,
        _directory: &'a str,
        _body: Value,
    ) -> RunnerFuture<'a, Value> {
        self.refuse()
    }

    fn configure<'a>(
        &'a self,
        _session_id: &'a str,
        _directory: &'a str,
        _choices: &'a crate::app::EngineChoices,
    ) -> RunnerFuture<'a, bool> {
        self.refuse()
    }

    fn execute_command<'a>(
        &'a self,
        _session_id: &'a str,
        _directory: &'a str,
        _command: SlashCommand<'a>,
    ) -> RunnerFuture<'a, Value> {
        self.refuse()
    }

    /// Nothing is running, so there is nothing to stop.
    fn abort<'a>(&'a self, _session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn rename<'a>(
        &'a self,
        _session_id: &'a str,
        _title: &'a str,
        _directory: &'a str,
    ) -> RunnerFuture<'a, bool> {
        self.refuse()
    }

    fn delete<'a>(&'a self, _session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, bool> {
        self.refuse()
    }
}
