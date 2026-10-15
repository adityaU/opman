//! A switched-off runner keeps listing its sessions and refuses everything else with
//! `RunnerDisabled`, which the web layer turns into a 409.

use serde_json::json;

use super::DisabledRunner;
use crate::runner::{Runner, RunnerDisabled, RunnerKind};
use crate::session_disk::SessionStore;

fn with_one_session() -> (tempfile::TempDir, DisabledRunner) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("acp_claude_sessions.json");
    let entry = json!([{ "id": "ses_old", "title": "Before it was off", "directory": "/proj",
                         "parent_id": "", "created": 1, "updated": 2 }]);
    std::fs::write(&path, entry.to_string()).expect("write");
    (dir, DisabledRunner::new(RunnerKind::Claude, Some(SessionStore::Acp(path))))
}

#[tokio::test]
async fn its_history_stays_listed() {
    let (_dir, runner) = with_one_session();
    assert!(runner.is_disabled());
    let sessions = runner.sessions("/proj").await.expect("lists");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "ses_old");
}

#[tokio::test]
async fn everything_that_would_run_it_is_refused_with_the_reason() {
    let (_dir, runner) = with_one_session();
    let refusals = [
        runner.send_message("ses_old", "/proj", json!({})).await.err(),
        runner.create_session("/proj", "new").await.err().map(anyhow::Error::from),
        runner.messages("ses_old", "/proj").await.err(),
        runner.ensure_started().await.err().map(anyhow::Error::from),
    ];
    for refusal in refusals {
        let refusal = refusal.expect("refused");
        let disabled = RunnerDisabled::find(&refusal).expect("a RunnerDisabled");
        assert_eq!(disabled.kind(), &RunnerKind::Claude);
        assert!(refusal.to_string().contains("turned off"));
    }
}

#[tokio::test]
async fn abort_has_nothing_to_stop() {
    let (_dir, runner) = with_one_session();
    assert!(runner.abort("ses_old", "/proj").await.is_ok());
}

#[tokio::test]
async fn without_a_store_it_lists_nothing() {
    let runner = DisabledRunner::new(RunnerKind::Opencode, None);
    assert!(runner.sessions("/proj").await.expect("lists").is_empty());
}
