//! A lazy runner lists its on-disk sessions without starting, and starts for one of them.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{LazyContext, LazyRunner, LazyStart};
use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerSession};
use crate::session_disk::SessionStore;

struct Live;

impl Runner for Live {
    fn kind(&self) -> RunnerKind {
        RunnerKind::Claude
    }

    fn create_session<'a>(&'a self, _: &'a str, title: &'a str) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            Ok(RunnerSession {
                id: "new".into(),
                title: title.into(),
            })
        })
    }

    fn messages<'a>(&'a self, session_id: &'a str, _: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move { Ok(json!([format!("replayed {session_id}")])) })
    }

    fn send_message<'a>(&'a self, _: &'a str, _: &'a str, _: Value) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({})) })
    }

    fn abort<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

/// A lazy ACP slot over a registry file holding one session in `/proj`.
fn lazy_over_disk() -> (tempfile::TempDir, LazyRunner, Arc<AtomicUsize>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("acp_claude_sessions.json");
    let entry = json!([{ "id": "ses_disk", "title": "Before restart", "directory": "/proj",
                         "parent_id": "", "created": 1, "updated": 2 }]);
    std::fs::write(&path, entry.to_string()).expect("write");
    let starts = Arc::new(AtomicUsize::new(0));
    let seen = starts.clone();
    let runner = LazyRunner::new(RunnerKind::Claude, LazyContext::new(), move || {
        seen.fetch_add(1, Ordering::SeqCst);
        async {
            Ok(LazyStart {
                runner: Arc::new(Live) as Arc<dyn Runner>,
                handle: None,
                engine: None,
                base_url: None,
            })
        }
    })
    .with_disk_sessions(Some(SessionStore::Acp(path)));
    (dir, runner, starts)
}

#[tokio::test]
async fn disk_sessions_are_listed_without_starting_the_runner() {
    let (_dir, runner, starts) = lazy_over_disk();
    let sessions = runner.sessions("/proj").await.expect("sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "ses_disk");
    assert_eq!(sessions[0].title, "Before restart");
    assert!(runner.sessions("/elsewhere").await.expect("sessions").is_empty());
    assert_eq!(runner.status("/proj").await.expect("status"), json!({}));
    assert_eq!(starts.load(Ordering::SeqCst), 0);
    assert!(!runner.is_started());
}

#[tokio::test]
async fn opening_a_listed_session_starts_its_runner() {
    let (_dir, runner, starts) = lazy_over_disk();
    runner.sessions("/proj").await.expect("sessions");
    let messages = runner.messages("ses_disk", "/proj").await.expect("messages");
    assert_eq!(messages, json!(["replayed ses_disk"]));
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert!(runner.is_started());
}

#[tokio::test]
async fn a_session_never_listed_from_disk_still_starts_nothing() {
    let (_dir, runner, starts) = lazy_over_disk();
    runner.sessions("/proj").await.expect("sessions");
    assert_eq!(runner.messages("ses_other", "/proj").await.expect("messages"), json!([]));
    assert!(!runner.rename("ses_other", "t", "/proj").await.expect("rename"));
    assert_eq!(starts.load(Ordering::SeqCst), 0);
}
