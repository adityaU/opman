//! Tests for the lazily started runner wrapper.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::app::EngineChoices;
use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerRegistry, RunnerSession};
use serde_json::{json, Value};

struct Stub(RunnerKind);

impl Runner for Stub {
    fn kind(&self) -> RunnerKind {
        self.0.clone()
    }

    fn create_session<'a>(
        &'a self,
        _directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            Ok(RunnerSession {
                id: "s1".to_string(),
                title: title.to_string(),
            })
        })
    }

    fn messages<'a>(
        &'a self,
        _session_id: &'a str,
        _directory: &'a str,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!(["from the real runner"])) })
    }

    fn send_message<'a>(
        &'a self,
        _session_id: &'a str,
        _directory: &'a str,
        _body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({ "ok": true })) })
    }

    fn agents<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!(["real-agent"])) })
    }

    fn abort<'a>(&'a self, _session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn counting(fail: usize) -> (Arc<LazyRunner>, Arc<AtomicUsize>) {
    let starts = Arc::new(AtomicUsize::new(0));
    let seen = starts.clone();
    let runner = LazyRunner::new(RunnerKind::Opencode, LazyContext::new(), move || {
        let seen = seen.clone();
        async move {
            let n = seen.fetch_add(1, Ordering::SeqCst);
            if n < fail {
                anyhow::bail!("binary not installed");
            }
            Ok(LazyStart {
                runner: Arc::new(Stub(RunnerKind::Opencode)) as Arc<dyn Runner>,
                handle: None,
                engine: None,
            })
        }
    });
    (Arc::new(runner), starts)
}

#[tokio::test]
async fn reads_and_identity_never_start_the_server() {
    let (runner, starts) = counting(0);

    assert_eq!(runner.kind(), RunnerKind::Opencode);
    assert!(runner.event_url().is_none());
    assert!(runner.event_receiver().is_none());
    assert_eq!(runner.agents("/p").await.unwrap(), json!([]));
    assert_eq!(runner.commands("/p").await.unwrap(), json!([]));
    assert_eq!(runner.status("/p").await.unwrap(), json!({}));
    assert_eq!(runner.messages("s1", "/p").await.unwrap(), json!([]));
    assert!(runner.sessions("/p").await.unwrap().is_empty());
    assert!(!runner.rename("s1", "t", "/p").await.unwrap());
    assert!(!runner.delete("s1", "/p").await.unwrap());
    assert!(!runner.reply_permission("r1", "allow").await.unwrap());
    assert!(!runner.reject_question("r1").await.unwrap());
    assert!(!runner
        .configure("s1", "/p", &EngineChoices::default())
        .await
        .unwrap());
    runner.abort("s1", "/p").await.unwrap();

    assert_eq!(starts.load(Ordering::SeqCst), 0);
    assert!(!runner.is_started());
}

#[tokio::test]
async fn first_session_starts_the_server_once() {
    let (runner, starts) = counting(0);

    let session = runner.create_session("/p", "hello").await.unwrap();
    assert_eq!(session.title, "hello");
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert!(runner.is_started());

    runner.send_message("s1", "/p", json!({})).await.unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(runner.agents("/p").await.unwrap(), json!(["real-agent"]));
    assert_eq!(
        runner.messages("s1", "/p").await.unwrap(),
        json!(["from the real runner"])
    );
}

#[tokio::test]
async fn send_message_alone_starts_the_server() {
    let (runner, starts) = counting(0);
    runner.send_message("s1", "/p", json!({})).await.unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn concurrent_first_use_starts_once() {
    let (runner, starts) = counting(0);
    let a = runner.clone();
    let b = runner.clone();
    let (first, second) = tokio::join!(
        async move { a.create_session("/p", "a").await },
        async move { b.send_message("s1", "/p", json!({})).await },
    );
    first.unwrap();
    second.unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_failed_start_is_retried() {
    let (runner, starts) = counting(1);
    assert!(runner.create_session("/p", "a").await.is_err());
    assert!(!runner.is_started());

    let session = runner.create_session("/p", "b").await.unwrap();
    assert_eq!(session.id, "s1");
    assert_eq!(starts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn pending_acp_agent_is_reported_until_started() {
    let runner = Arc::new(
        LazyRunner::new(RunnerKind::Opencode, LazyContext::new(), || async {
            Ok(LazyStart {
                runner: Arc::new(Stub(RunnerKind::Opencode)) as Arc<dyn Runner>,
                handle: None,
                engine: None,
            })
        })
        .for_acp_agent("gemini"),
    );
    assert_eq!(runner.pending_acp_agent().as_deref(), Some("gemini"));
    runner.create_session("/p", "a").await.unwrap();
    assert_eq!(runner.pending_acp_agent(), None);
}

#[tokio::test]
async fn starting_installs_the_real_runner_and_fires_the_hook() {
    let ctx = LazyContext::new();
    let lazy = Arc::new(LazyRunner::new(
        RunnerKind::Opencode,
        ctx.clone(),
        || async {
            Ok(LazyStart {
                runner: Arc::new(Stub(RunnerKind::Opencode)) as Arc<dyn Runner>,
                handle: None,
                engine: None,
            })
        },
    ));
    let mut runners = std::collections::HashMap::new();
    runners.insert(RunnerKind::Opencode, lazy.clone() as Arc<dyn Runner>);
    let registry = Arc::new(RunnerRegistry::new(RunnerKind::Opencode, runners));
    ctx.set_registry(&registry);

    let notified = Arc::new(AtomicUsize::new(0));
    let seen = notified.clone();
    registry.set_on_runner_started(Arc::new(move |_kind, _runner| {
        seen.fetch_add(1, Ordering::SeqCst);
    }));

    let cold_messages: Value = registry.messages("s1", "/p").await.unwrap();
    assert_eq!(cold_messages, json!([]));
    assert_eq!(notified.load(Ordering::SeqCst), 0);

    lazy.create_session("/p", "a").await.unwrap();
    assert_eq!(notified.load(Ordering::SeqCst), 1);
    let started_messages: Value = registry.messages("s1", "/p").await.unwrap();
    assert_eq!(started_messages, json!(["from the real runner"]));
}
