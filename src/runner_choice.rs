//! Which runner answers when nobody names one, and what a switched-off runner says.
//!
//! Two different questions used to share one field. The *primary* runner is the one opman
//! started on: its URL is the process-wide base URL the TUI and the background pollers
//! talk to, so it cannot change while the process runs. The *default* runner is where a new
//! session goes — and that has to follow the settings page, because a runner can now be
//! switched off, and a default that names a switched-off runner sends every new session to
//! a refusal.

use std::fmt::{Display, Formatter};
use std::sync::Arc;

use super::{Runner, RunnerKind, RunnerRegistry};

/// A request reached a runner that is switched off in settings.
///
/// A distinct type rather than a message so the web layer can answer 409 with a code the
/// browser recognises, and offer to turn the runner back on, instead of a generic 500.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerDisabled {
    kind: RunnerKind,
}

impl RunnerDisabled {
    pub fn new(kind: RunnerKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &RunnerKind {
        &self.kind
    }

    /// Find one anywhere in an error chain: registry calls wrap runner errors in context.
    pub fn find(error: &anyhow::Error) -> Option<&Self> {
        error.chain().find_map(|cause| cause.downcast_ref::<Self>())
    }
}

impl Display for RunnerDisabled {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "The {} runner is turned off. Turn it on in Settings → Agents to use it.",
            self.kind.display_name()
        )
    }
}

impl std::error::Error for RunnerDisabled {}

impl RunnerRegistry {
    /// The runner opman started on. See the module docs for why it is fixed.
    pub fn primary_kind(&self) -> RunnerKind {
        self.primary.clone()
    }

    /// Where a new session goes when the caller names no runner.
    ///
    /// The user's choice if it can be used, else the primary, else the first runner on
    /// offer. Only when nothing is usable at all is the primary returned regardless — and
    /// then the request is refused with a reason rather than routed somewhere surprising.
    pub fn default_kind(&self) -> RunnerKind {
        let preferred = match self.preferred.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        preferred
            .into_iter()
            .chain(std::iter::once(self.primary.clone()))
            .find(|kind| self.usable(kind))
            .or_else(|| self.available().into_iter().next())
            .unwrap_or_else(|| self.primary.clone())
    }

    /// Record the user's default. `None` returns the choice to the primary runner.
    pub fn set_preferred(&self, kind: Option<RunnerKind>) {
        match self.preferred.write() {
            Ok(mut guard) => *guard = kind,
            Err(poisoned) => *poisoned.into_inner() = kind,
        }
    }

    /// The slot exists and is switched off.
    pub fn is_disabled(&self, kind: &RunnerKind) -> bool {
        self.snapshot()
            .get(kind)
            .is_some_and(|runner| runner.is_disabled())
    }

    /// The slot exists and is switched on — something a session can actually run in.
    pub fn usable(&self, kind: &RunnerKind) -> bool {
        self.snapshot()
            .get(kind)
            .is_some_and(|runner| !runner.is_disabled())
    }

    /// Install a freshly started runner unless its slot was switched off meanwhile.
    ///
    /// A runner can take seconds to come up, and the user can turn it off in that window.
    /// Checking and installing under the one write keeps the start from overwriting the
    /// disabled placeholder and bringing the runner back on its own.
    pub fn install_unless_disabled(&self, kind: RunnerKind, runner: Arc<dyn Runner>) -> bool {
        let mut installed = false;
        self.mutate(|runners| {
            if runners.get(&kind).is_some_and(|current| current.is_disabled()) {
                return;
            }
            runners.insert(kind, runner);
            installed = true;
        });
        installed
    }
}
