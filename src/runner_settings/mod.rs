//! Switching the built-in runners (OpenCode, Claude Code) on and off, and choosing the
//! default runner, from the settings page.
//!
//! ACP agents are switched in `acp.json` with the rest of their definition; these two have
//! nothing to define but whether they run, so they keep `runners.json` ([`config`]). A
//! runner switched off is stopped and its slot replaced by a [`DisabledRunner`]-style
//! placeholder ([`SlotFactory::disabled`]) that still lists its sessions and refuses
//! everything else, so its history stays visible and nothing can boot it by accident.
//!
//! [`DisabledRunner`]: crate::runner_lazy::DisabledRunner

mod boot;
mod builtin;
mod config;
mod factory;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{bail, Result};
use serde::Serialize;

use crate::runner::{Runner, RunnerKind, RunnerRegistry};

pub use boot::choose_primary;
pub use builtin::BuiltinRunner;
pub use config::{config_path, load, save, RunnersDocument};
pub use factory::{BuiltinFactory, SlotFactory};

/// One built-in runner as the settings page shows it.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinView {
    pub id: &'static str,
    pub name: &'static str,
    /// Its binary answers `--version`; one that does not cannot be switched on.
    pub installed: bool,
    pub enabled: bool,
    /// opman started on it, so it serves the TUI and the process-wide base URL.
    pub primary: bool,
}

/// The settings page's view of every runner choice.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnersView {
    pub builtin: Vec<BuiltinView>,
    /// Where a new session goes when nobody names a runner.
    pub default: String,
    /// The runners that can be the default: served and switched on.
    pub choices: Vec<String>,
}

pub struct RunnerSettings {
    path: Option<PathBuf>,
    document: Mutex<RunnersDocument>,
    installed: BTreeSet<BuiltinRunner>,
    primary: RunnerKind,
    factory: Arc<dyn SlotFactory>,
}

impl RunnerSettings {
    /// Read `runners.json`, see which built-in runners are installed, and decide which
    /// runner opman starts on. `cli` is what `--backend` asked for.
    pub fn boot(path: Option<PathBuf>, factory: Arc<dyn SlotFactory>, cli: RunnerKind) -> Self {
        let document = path.as_deref().map(load).unwrap_or_default();
        let installed: BTreeSet<BuiltinRunner> = BuiltinRunner::ALL
            .into_iter()
            .filter(|runner| factory.installed(*runner))
            .collect();
        let usable: Vec<RunnerKind> = installed
            .iter()
            .filter(|runner| !document.is_disabled(**runner))
            .map(|runner| runner.kind())
            .collect();
        // Only a built-in runner can be switched off here; any other CLI choice stands.
        let cli_enabled = BuiltinRunner::from_kind(&cli)
            .is_none_or(|runner| usable.contains(&runner.kind()));
        let saved = document.default.as_deref().and_then(RunnerKind::parse);
        let primary = choose_primary(cli, cli_enabled, saved.as_ref(), &usable);
        Self {
            path,
            document: Mutex::new(document),
            installed,
            primary,
            factory,
        }
    }

    pub fn primary(&self) -> &RunnerKind {
        &self.primary
    }

    /// The default the user saved, if any.
    pub fn saved_default(&self) -> Option<RunnerKind> {
        self.document().default.as_deref().and_then(RunnerKind::parse)
    }

    /// A slot for every installed built-in runner: a lazy one, or a placeholder when it
    /// is switched off. The primary always gets a real slot, installed or not, so a
    /// missing binary is reported by the start that needed it rather than hidden.
    pub fn slots(&self) -> Vec<(RunnerKind, Arc<dyn Runner>)> {
        let document = self.document();
        BuiltinRunner::ALL
            .into_iter()
            .filter(|runner| self.installed.contains(runner) || runner.kind() == self.primary)
            .map(|runner| {
                let slot = if document.is_disabled(runner) {
                    self.factory.disabled(runner)
                } else {
                    self.factory.build(runner, runner.kind() == self.primary)
                };
                (runner.kind(), slot)
            })
            .collect()
    }

    pub fn view(&self, registry: &RunnerRegistry) -> RunnersView {
        let document = self.document();
        let builtin = BuiltinRunner::ALL
            .into_iter()
            .map(|runner| BuiltinView {
                id: runner.id(),
                name: runner.display_name(),
                installed: self.installed.contains(&runner),
                enabled: !document.is_disabled(runner),
                primary: runner.kind() == self.primary,
            })
            .collect();
        RunnersView {
            builtin,
            default: registry.default_kind().display_name().into_owned(),
            choices: registry
                .available()
                .iter()
                .map(|kind| kind.display_name().into_owned())
                .collect(),
        }
    }

    /// Switch a built-in runner on or off. Off stops whatever it has running; on installs
    /// a slot that starts it on first use. A default that pointed at a runner now off
    /// moves to the next usable one, so new sessions never go to a refusal.
    pub fn set_enabled(&self, registry: &RunnerRegistry, runner: BuiltinRunner, on: bool) -> Result<()> {
        if !self.installed.contains(&runner) {
            bail!("{} is not installed on this machine", runner.display_name());
        }
        let kind = runner.kind();
        let primary = kind == self.primary;
        let mut document = self.document();
        if on {
            document.disabled.remove(&runner);
            self.persist(&document)?;
            registry.install(kind, self.factory.build(runner, primary));
            return Ok(());
        }
        document.disabled.insert(runner);
        // Refuse new requests before the running server is taken away.
        registry.install(kind.clone(), self.factory.disabled(runner));
        self.factory.stop(runner, primary);
        let default_was_this = document
            .default
            .as_deref()
            .and_then(RunnerKind::parse)
            .map_or(primary, |saved| saved == kind);
        if default_was_this {
            let next = registry.default_kind();
            document.default = registry.usable(&next).then(|| next.display_name().into_owned());
            registry.set_preferred(registry.usable(&next).then_some(next));
        }
        self.persist(&document)
    }

    /// Make `kind` where new sessions go. It has to be served and switched on.
    pub fn set_default(&self, registry: &RunnerRegistry, kind: RunnerKind) -> Result<()> {
        if !registry.usable(&kind) {
            bail!("{} is not available", kind.display_name());
        }
        let mut document = self.document();
        document.default = Some(kind.display_name().into_owned());
        self.persist(&document)?;
        registry.set_preferred(Some(kind));
        Ok(())
    }

    fn document(&self) -> std::sync::MutexGuard<'_, RunnersDocument> {
        self.document.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn persist(&self, document: &RunnersDocument) -> Result<()> {
        match &self.path {
            Some(path) => save(path, document),
            None => Ok(()),
        }
    }
}

static GLOBAL: OnceLock<Arc<RunnerSettings>> = OnceLock::new();

/// Make `settings` the ones the web layer edits. Set once, at startup.
pub fn install_global(settings: Arc<RunnerSettings>) {
    let _ = GLOBAL.set(settings);
}

/// The settings opman started with; `None` in tests and tools that build no runners.
pub fn global() -> Option<&'static Arc<RunnerSettings>> {
    GLOBAL.get()
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
