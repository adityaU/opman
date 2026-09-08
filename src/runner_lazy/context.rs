use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::acp_engine::supervisor::AcpSupervisor;
use crate::acp_engine::AcpEngine;
use crate::runner::{Runner, RunnerRegistry};
use crate::server::ServerHandle;

/// Everything a successful start produced.
pub struct LazyStart {
    /// The real runner, which replaces the wrapper in the registry.
    pub runner: Arc<dyn Runner>,
    /// The child process to kill on shutdown, when the engine spawns one.
    pub handle: Option<ServerHandle>,
    /// The ACP agent id and engine, so the supervisor can own it from here on.
    pub engine: Option<(String, Arc<AcpEngine>)>,
}

/// The process-wide bits a lazily started runner has to join once it is alive.
///
/// Startup builds the runner map before the registry and the supervisor exist, so both are
/// filled in afterwards rather than passed to the constructor. They are held weakly: the
/// registry owns the runners, and a runner holding the registry back would keep the whole
/// graph alive for the life of the process.
#[derive(Default)]
pub struct LazyContext {
    registry: OnceLock<Weak<RunnerRegistry>>,
    supervisor: OnceLock<Weak<AcpSupervisor>>,
    handles: Mutex<Vec<ServerHandle>>,
}

impl LazyContext {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn set_registry(&self, registry: &Arc<RunnerRegistry>) {
        let _ = self.registry.set(Arc::downgrade(registry));
    }

    pub fn set_supervisor(&self, supervisor: &Arc<AcpSupervisor>) {
        let _ = self.supervisor.set(Arc::downgrade(supervisor));
    }

    /// Register a child process with the shutdown path. Servers started after boot have to
    /// end up in the same list the Ctrl+C handler walks, or they outlive opman.
    pub fn push_handle(&self, handle: ServerHandle) {
        if let Ok(mut handles) = self.handles.lock() {
            handles.push(handle);
        }
    }

    /// The handles registered so far, for the shutdown path to kill.
    pub fn handles(&self) -> Vec<ServerHandle> {
        match self.handles.lock() {
            Ok(handles) => handles.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(crate) fn registry(&self) -> Option<Arc<RunnerRegistry>> {
        self.registry.get().and_then(Weak::upgrade)
    }

    pub(crate) fn supervisor(&self) -> Option<Arc<AcpSupervisor>> {
        self.supervisor.get().and_then(Weak::upgrade)
    }
}
