//! A scripted runner, so the manager's own logic can be tested without a live agent.
//!
//! It implements the handful of trait methods the manager actually reaches and records
//! what it was asked to do; everything else keeps the trait's default. The provider
//! payload deliberately uses the shape the *live* runners emit — efforts as the keys of a
//! `variants` map — because reading that shape is the thing being tested.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::app::{EngineChoices, SessionInfo};
use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerRegistry, RunnerSession};

/// The project every manager test works in. Absolute, which is all a project directory has
/// to be; nothing here touches the filesystem.
pub(super) const DIR: &str = "/work";

/// A manager wired to two fake runners, and handles on both.
///
/// Two rather than one because the interesting paths are the ones where the runners
/// differ: a send that switches runner, a default that is not the caller's own.
pub(super) struct Harness {
    pub(super) state: super::ManagerState,
    pub(super) opencode: Arc<FakeRunner>,
    pub(super) claude: Arc<FakeRunner>,
}

impl Harness {
    pub(super) fn new() -> Self {
        let opencode = Arc::new(FakeRunner::new(RunnerKind::Opencode));
        let claude = Arc::new(FakeRunner::new(RunnerKind::Claude));
        let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
        runners.insert(RunnerKind::Opencode, opencode.clone());
        runners.insert(RunnerKind::Claude, claude.clone());
        Self {
            state: super::ManagerState {
                registry: Arc::new(RunnerRegistry::new(RunnerKind::Opencode, runners)),
                parents: Arc::new(Mutex::new(HashMap::new())),
                queues: Arc::new(Mutex::new(HashMap::new())),
            },
            opencode,
            claude,
        }
    }

    /// Run one request the way the socket handler would.
    pub(super) async fn call(&self, request: Value) -> Result<Value> {
        let request = serde_json::from_value(request)?;
        super::handle_manager_request(&self.state, request).await
    }

    /// A session on `kind`, already configured with `permission` when one is given.
    pub(super) async fn session(&self, kind: RunnerKind, permission: &str) -> String {
        let session = self
            .state
            .registry
            .create_session(kind, DIR, "parent")
            .await
            .expect("create");
        let choices = crate::app::EngineChoices::from_parts(None, None, None, Some(permission));
        self.state
            .registry
            .configure(&session.id, DIR, &choices)
            .await
            .expect("configure");
        session.id
    }
}

/// One recorded `send_message`.
#[derive(Clone, Debug)]
pub(super) struct Sent {
    pub(super) session: String,
    pub(super) body: Value,
}

impl Sent {
    /// The prompt text, which is what almost every assertion is about.
    pub(super) fn text(&self) -> &str {
        self.body["parts"][0]["text"].as_str().unwrap_or_default()
    }
}

/// The modes each fake runner publishes, in the live `/provider` shape.
///
/// Deliberately disjoint between the two: the mode names are one runner's vocabulary, and
/// a `bypassPermissions` carried onto opencode is the mistake worth catching.
fn permission_modes(kind: &RunnerKind) -> Value {
    let modes: &[&str] = match kind {
        RunnerKind::Claude => &["default", "acceptEdits", "bypassPermissions"],
        _ => &["build", "plan"],
    };
    modes
        .iter()
        .map(|mode| json!({ "value": mode, "label": mode, "description": "" }))
        .collect()
}

pub(super) struct FakeRunner {
    kind: RunnerKind,
    counter: AtomicUsize,
    busy: AtomicBool,
    failing: AtomicBool,
    stall: Mutex<Option<Duration>>,
    ids: Mutex<Vec<String>>,
    sends: Mutex<Vec<Sent>>,
    aborted: Mutex<Vec<String>>,
    transcript: Mutex<Value>,
    /// What each session has been configured to run as — written by `configure`, read back
    /// by `sessions`, which is how a real engine behaves and what the inheritance path
    /// depends on.
    engines: Mutex<HashMap<String, EngineChoices>>,
}

impl FakeRunner {
    pub(super) fn new(kind: RunnerKind) -> Self {
        Self {
            kind,
            counter: AtomicUsize::new(0),
            busy: AtomicBool::new(false),
            failing: AtomicBool::new(false),
            stall: Mutex::new(None),
            ids: Mutex::new(Vec::new()),
            sends: Mutex::new(Vec::new()),
            aborted: Mutex::new(Vec::new()),
            transcript: Mutex::new(json!([])),
            engines: Mutex::new(HashMap::new()),
        }
    }

    /// What one session is configured to run as, as `configure` left it.
    pub(super) async fn engine_of(&self, session_id: &str) -> Option<EngineChoices> {
        self.engines.lock().await.get(session_id).cloned()
    }

    /// Whether every session this runner owns reports a turn in flight.
    pub(super) fn set_busy(&self, busy: bool) {
        self.busy.store(busy, Ordering::SeqCst);
    }

    /// Whether the next `send_message` fails.
    pub(super) fn set_failing(&self, failing: bool) {
        self.failing.store(failing, Ordering::SeqCst);
    }

    /// Hold every `send_message` open this long — a runner that has wedged.
    pub(super) async fn set_stall(&self, stall: Option<Duration>) {
        *self.stall.lock().await = stall;
    }

    pub(super) async fn set_transcript(&self, transcript: Value) {
        *self.transcript.lock().await = transcript;
    }

    pub(super) async fn sends(&self) -> Vec<Sent> {
        self.sends.lock().await.clone()
    }

    pub(super) async fn aborted(&self) -> Vec<String> {
        self.aborted.lock().await.clone()
    }
}

impl Runner for FakeRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }

    fn create_session<'a>(
        &'a self,
        _directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            let n = self.counter.fetch_add(1, Ordering::SeqCst);
            let id = format!("ses_{}_{n}", self.kind.display_name());
            self.ids.lock().await.push(id.clone());
            Ok(RunnerSession {
                id,
                title: title.to_string(),
            })
        })
    }

    fn sessions<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Vec<SessionInfo>> {
        Box::pin(async move {
            let engines = self.engines.lock().await;
            Ok(self
                .ids
                .lock()
                .await
                .iter()
                .map(|id| SessionInfo {
                    id: id.clone(),
                    title: format!("{id} title"),
                    directory: directory.to_string(),
                    engine: engines.get(id).cloned().unwrap_or_default(),
                    ..SessionInfo::default()
                })
                .collect())
        })
    }

    fn configure<'a>(
        &'a self,
        session_id: &'a str,
        _directory: &'a str,
        choices: &'a EngineChoices,
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            self.engines
                .lock()
                .await
                .insert(session_id.to_string(), choices.clone());
            Ok(true)
        })
    }

    fn messages<'a>(
        &'a self,
        _session_id: &'a str,
        _directory: &'a str,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move { Ok(self.transcript.lock().await.clone()) })
    }

    /// The `/session/status` envelope: only non-idle sessions need appear, so an idle
    /// runner answers with an empty map.
    fn status<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            if !self.busy.load(Ordering::SeqCst) {
                return Ok(json!({}));
            }
            let mut status = serde_json::Map::new();
            for id in self.ids.lock().await.iter() {
                status.insert(id.clone(), json!({ "type": "busy" }));
            }
            Ok(Value::Object(status))
        })
    }

    /// Two providers in the live shape: efforts live under `variants`, and one model
    /// belongs to a provider that is not connected.
    fn providers<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            Ok(json!({
                "all": [
                    { "id": "openai", "name": "OpenAI", "models": {
                        "gpt-5.6-luna": { "id": "gpt-5.6-luna", "name": "GPT-5.6 Luna", "variants": {
                            "low": {}, "medium": {}, "high": {},
                        }},
                    }},
                    { "id": "claude", "name": "Claude", "models": {
                        "haiku": { "id": "haiku", "name": "Haiku" },
                    }},
                    { "id": "unplugged", "name": "Unplugged", "models": {
                        "ghost": { "id": "ghost", "name": "Ghost", "reasoningEfforts": ["low"] },
                    }},
                ],
                "connected": ["openai", "claude"],
                "default": { "openai": "gpt-5.6-luna" },
                "permissionModes": permission_modes(&self.kind),
            }))
        })
    }

    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        _directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            if let Some(stall) = *self.stall.lock().await {
                tokio::time::sleep(stall).await;
            }
            if self.failing.load(Ordering::SeqCst) {
                bail!("fake runner refused the send");
            }
            self.sends.lock().await.push(Sent {
                session: session_id.to_string(),
                body,
            });
            Ok(json!({ "ok": true }))
        })
    }

    fn abort<'a>(&'a self, session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async move {
            self.aborted.lock().await.push(session_id.to_string());
            Ok(())
        })
    }
}
