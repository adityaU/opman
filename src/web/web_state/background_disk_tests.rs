//! Boot with no runner started: the sidebar is filled from the runners' own stores.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::api::ApiClient;
use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerRegistry, RunnerSession};
use crate::runner_lazy::{LazyContext, LazyRunner};
use crate::session_disk::SessionStore;
use crate::web::test_support::{send_json, test_router, test_server_state_with_projects};
use crate::web::web_state::background_hydration::StartupHydration;
use crate::web::web_state::WebStateHandle;

const PROJECT: &str = "/proj";

/// An opencode database and an ACP registry, one session each in [`PROJECT`].
fn stores(dir: &Path) -> (SessionStore, SessionStore) {
    let opencode = dir.join("opencode");
    std::fs::create_dir_all(&opencode).expect("mkdir");
    let conn = rusqlite::Connection::open(opencode.join("opencode.db")).expect("db");
    conn.execute_batch(
        "CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_id TEXT,
            directory TEXT NOT NULL, title TEXT NOT NULL, time_created INTEGER NOT NULL,
            time_updated INTEGER NOT NULL);
         INSERT INTO session VALUES ('ses_oc', 'p1', NULL, '/proj', 'From opencode', 1, 20);",
    )
    .expect("fixture");
    let acp = dir.join("acp_claude_sessions.json");
    let entry = json!([{ "id": "ses_acp", "title": "From claude ACP", "directory": PROJECT,
                         "parent_id": "", "created": 5, "updated": 10 }]);
    std::fs::write(&acp, entry.to_string()).expect("write");
    (SessionStore::Opencode(opencode), SessionStore::Acp(acp))
}

/// A lazy slot whose start is counted, so a test can prove it never ran.
fn lazy(kind: RunnerKind, store: SessionStore, starts: &Arc<AtomicUsize>) -> Arc<dyn Runner> {
    let starts = starts.clone();
    Arc::new(
        LazyRunner::new(kind, LazyContext::new(), move || {
            starts.fetch_add(1, Ordering::SeqCst);
            async { anyhow::bail!("no runner may start while listing") }
        })
        .with_disk_sessions(Some(store)),
    )
}

fn registry(dir: &Path, starts: &Arc<AtomicUsize>) -> Arc<RunnerRegistry> {
    let (opencode, acp) = stores(dir);
    let mut runners = HashMap::new();
    runners.insert(RunnerKind::Opencode, lazy(RunnerKind::Opencode, opencode, starts));
    runners.insert(RunnerKind::Claude, lazy(RunnerKind::Claude, acp, starts));
    Arc::new(RunnerRegistry::new(RunnerKind::Opencode, runners))
}

async fn boot(handle: &WebStateHandle) -> StartupHydration {
    let client = ApiClient::new();
    crate::app::TEST_BASE_URL_UNSET
        .scope((), handle.session_poll_startup(&client))
        .await
}

async fn ids(handle: &WebStateHandle) -> Vec<String> {
    let state = handle.inner.read().await;
    state.projects[0].sessions.iter().map(|s| s.id.clone()).collect()
}

#[tokio::test]
async fn boot_without_a_runner_lists_every_runner_from_disk_and_starts_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let starts = Arc::new(AtomicUsize::new(0));
    let mut handle = WebStateHandle::new_test_with_projects(vec![("p".into(), PROJECT.into())]);
    handle.attach_runner_registry_for_test(registry(dir.path(), &starts));

    assert_eq!(boot(&handle).await, StartupHydration::NoRunner);
    let mut listed = ids(&handle).await;
    listed.sort();
    assert_eq!(listed, ["ses_acp", "ses_oc"]);
    {
        let state = handle.inner.read().await;
        assert_eq!(state.session_runners.get("ses_acp").map(String::as_str), Some("claude"));
        assert_eq!(state.session_runners.get("ses_oc"), None, "default rows carry no label");
        assert_eq!(state.projects[0].active_session, None, "boot must not open a session");
    }

    // An unstarted runner answers status with nothing running: disk rows are idle.
    let (became_busy, _) = handle.status_sweep_once().await;
    assert!(became_busy.is_empty());
    assert!(handle.inner.read().await.busy_sessions.is_empty());
    assert_eq!(starts.load(Ordering::SeqCst), 0);
}

struct Live;

impl Runner for Live {
    fn kind(&self) -> RunnerKind {
        RunnerKind::Claude
    }

    fn sessions<'a>(&'a self, _: &'a str) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        Box::pin(async {
            Ok(vec![crate::web::test_support::test_session("ses_live", "", PROJECT, 30)])
        })
    }

    fn create_session<'a>(&'a self, _: &'a str, title: &'a str) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            Ok(RunnerSession {
                id: "new".into(),
                title: title.into(),
            })
        })
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

#[tokio::test]
async fn a_runner_starting_replaces_only_the_rows_it_owns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let starts = Arc::new(AtomicUsize::new(0));
    let mut handle = WebStateHandle::new_test_with_projects(vec![("p".into(), PROJECT.into())]);
    handle.attach_runner_registry_for_test(registry(dir.path(), &starts));
    boot(&handle).await;

    let live: Arc<dyn Runner> = Arc::new(Live);
    handle.refresh_started_runner(&RunnerKind::Claude, &live).await;

    let mut listed = ids(&handle).await;
    listed.sort();
    // The live list wins for claude; opencode's disk row, owned by a runner that is still
    // not started, stays put.
    assert_eq!(listed, ["ses_live", "ses_oc"]);
}

#[tokio::test]
async fn state_and_sessions_endpoints_serve_disk_sessions_before_any_runner_starts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let starts = Arc::new(AtomicUsize::new(0));
    let registry = registry(dir.path(), &starts);
    let mut state = test_server_state_with_projects(vec![("p".into(), PathBuf::from(PROJECT))]);
    state.runner_registry = registry.clone();
    state.web_state.attach_runner_registry_for_test(registry);
    boot(&state.web_state).await;

    let (status, body) = send_json(test_router(state.clone()), "GET", "/api/state", None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).expect("json");
    let project = &v["projects"][0];
    assert_eq!(project["session_count"], 2);
    let rows = project["sessions"].as_array().expect("sessions");
    let row = |id: &str| rows.iter().find(|row| row["id"] == id).cloned();
    let acp = row("ses_acp").expect("ACP row");
    assert_eq!(acp["title"], "From claude ACP");
    assert_eq!(acp["runner"], "claude");
    let opencode = row("ses_oc").expect("opencode row");
    assert_eq!(opencode["runner"], "opencode");
    assert_eq!(opencode["time"]["updated"], 20);
    assert_eq!(v["startup_ready"], true);
    assert_eq!(project["busy_sessions"], json!([]));

    let (status, body) = send_json(
        test_router(state),
        "GET",
        "/api/sessions?project=0&offset=1&limit=1",
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let page: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(page["sessions"][0]["id"], "ses_acp", "older row on the second page: {page}");
    assert_eq!(starts.load(Ordering::SeqCst), 0);
}
