//! Switching built-in runners on and off, the primary choice at boot, and the default.
//! A fake factory stands in for `opencode serve` and the embedded claude engine.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::*;
use crate::runner::{RunnerFuture, RunnerSession};
use crate::runner_lazy::DisabledRunner;

/// A runner that is on and answers trivially.
struct On(RunnerKind);

impl Runner for On {
    fn kind(&self) -> RunnerKind {
        self.0.clone()
    }
    fn create_session<'a>(&'a self, _: &'a str, title: &'a str) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move { Ok(RunnerSession { id: "s".into(), title: title.into() }) })
    }
    fn messages<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!([])) })
    }
    fn send_message<'a>(&'a self, _: &'a str, _: &'a str, _: Value) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({})) })
    }
    fn abort<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct Fake {
    missing: Vec<BuiltinRunner>,
    stopped: Mutex<Vec<(BuiltinRunner, bool)>>,
}

impl SlotFactory for Fake {
    fn installed(&self, runner: BuiltinRunner) -> bool {
        !self.missing.contains(&runner)
    }
    fn build(&self, runner: BuiltinRunner, _primary: bool) -> Arc<dyn Runner> {
        Arc::new(On(runner.kind()))
    }
    fn disabled(&self, runner: BuiltinRunner) -> Arc<dyn Runner> {
        Arc::new(DisabledRunner::new(runner.kind(), None))
    }
    fn stop(&self, runner: BuiltinRunner, primary: bool) {
        self.stopped.lock().expect("lock").push((runner, primary));
    }
}

fn file(dir: &tempfile::TempDir, body: Option<&str>) -> PathBuf {
    let path = dir.path().join("runners.json");
    if let Some(body) = body {
        std::fs::write(&path, body).expect("write");
    }
    path
}

fn registry_for(settings: &RunnerSettings) -> RunnerRegistry {
    let slots: HashMap<RunnerKind, Arc<dyn Runner>> = settings.slots().into_iter().collect();
    let registry = RunnerRegistry::new(settings.primary().clone(), slots);
    registry.set_preferred(settings.saved_default());
    registry
}

#[test]
fn the_cli_choice_is_primary_while_it_is_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = RunnerSettings::boot(Some(file(&dir, None)), Arc::new(Fake::default()), RunnerKind::Opencode);
    assert_eq!(settings.primary(), &RunnerKind::Opencode);
}

#[test]
fn a_switched_off_cli_choice_gives_way_to_a_runner_that_is_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = file(&dir, Some(r#"{"disabled":["opencode"]}"#));
    let settings = RunnerSettings::boot(Some(path), Arc::new(Fake::default()), RunnerKind::Opencode);
    assert_eq!(settings.primary(), &RunnerKind::ClaudeCode);
    let registry = registry_for(&settings);
    assert!(registry.is_disabled(&RunnerKind::Opencode));
    assert!(registry.usable(&RunnerKind::ClaudeCode));
    assert_eq!(registry.default_kind(), RunnerKind::ClaudeCode);
}

#[test]
fn switching_off_stops_it_saves_it_and_moves_the_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = file(&dir, None);
    let factory = Arc::new(Fake::default());
    let settings = RunnerSettings::boot(Some(path.clone()), factory.clone(), RunnerKind::Opencode);
    let registry = registry_for(&settings);

    settings.set_enabled(&registry, BuiltinRunner::Opencode, false).expect("off");
    assert!(registry.is_disabled(&RunnerKind::Opencode));
    assert_eq!(*factory.stopped.lock().expect("lock"), vec![(BuiltinRunner::Opencode, true)]);
    // New sessions must not go to a refusal.
    assert_eq!(registry.default_kind(), RunnerKind::ClaudeCode);
    let saved = load(&path);
    assert!(saved.is_disabled(BuiltinRunner::Opencode));
    assert_eq!(saved.default.as_deref(), Some("claude-code"));

    settings.set_enabled(&registry, BuiltinRunner::Opencode, true).expect("on");
    assert!(registry.usable(&RunnerKind::Opencode));
    assert!(!load(&path).is_disabled(BuiltinRunner::Opencode));
}

#[test]
fn a_runner_that_is_not_installed_cannot_be_switched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let factory = Arc::new(Fake { missing: vec![BuiltinRunner::ClaudeCode], ..Fake::default() });
    let settings = RunnerSettings::boot(Some(file(&dir, None)), factory, RunnerKind::Opencode);
    let registry = registry_for(&settings);
    assert!(settings.set_enabled(&registry, BuiltinRunner::ClaudeCode, true).is_err());
    let view = settings.view(&registry);
    let claude = view.builtin.iter().find(|row| row.id == "claude-code").expect("listed");
    assert!(!claude.installed);
}

#[test]
fn the_default_must_be_usable_and_is_saved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = file(&dir, None);
    let settings = RunnerSettings::boot(Some(path.clone()), Arc::new(Fake::default()), RunnerKind::Opencode);
    let registry = registry_for(&settings);

    settings.set_default(&registry, RunnerKind::ClaudeCode).expect("usable");
    assert_eq!(registry.default_kind(), RunnerKind::ClaudeCode);
    assert_eq!(load(&path).default.as_deref(), Some("claude-code"));

    settings.set_enabled(&registry, BuiltinRunner::ClaudeCode, false).expect("off");
    assert!(settings.set_default(&registry, RunnerKind::ClaudeCode).is_err());
    assert_eq!(registry.default_kind(), RunnerKind::Opencode);
}

#[test]
fn the_view_says_what_each_runner_is() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = file(&dir, Some(r#"{"disabled":["claude-code"]}"#));
    let settings = RunnerSettings::boot(Some(path), Arc::new(Fake::default()), RunnerKind::Opencode);
    let registry = registry_for(&settings);
    let view = settings.view(&registry);
    assert_eq!(view.default, "opencode");
    assert_eq!(view.choices, vec!["opencode".to_string()]);
    assert_eq!(
        view.builtin,
        vec![
            BuiltinView { id: "opencode", name: "OpenCode", installed: true, enabled: true, primary: true },
            BuiltinView { id: "claude-code", name: "Claude Code", installed: true, enabled: false, primary: false },
        ]
    );
}

#[test]
fn a_bad_file_means_everything_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(load(&file(&dir, Some("{not json"))), RunnersDocument::default());
    let unknown = load(&file(&dir, Some(r#"{"disabled":["nope","opencode"]}"#)));
    assert!(unknown.is_disabled(BuiltinRunner::Opencode));
    assert_eq!(unknown.disabled.len(), 1);
}
