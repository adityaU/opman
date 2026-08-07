use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use std::sync::Mutex;

// ── Test helpers ────────────────────────────────────────────────────

fn ensure_base_url() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:9".to_string());
}

fn tc() -> WebThemeColors {
    WebThemeColors {
        primary: "#111111".into(),
        secondary: "#222222".into(),
        accent: "#333333".into(),
        background: "#444444".into(),
        background_panel: "#555555".into(),
        background_element: "#666666".into(),
        text: "#777777".into(),
        text_muted: "#888888".into(),
        border: "#999999".into(),
        border_active: "#aaaaaa".into(),
        border_subtle: "#bbbbbb".into(),
        error: "#cccccc".into(),
        warning: "#dddddd".into(),
        success: "#eeeeee".into(),
        info: "#ffffff".into(),
    }
}

fn theme() -> WebThemePair {
    WebThemePair {
        name: "test-theme".to_string(),
        dark: tc(),
        light: tc(),
    }
}

fn sess(id: &str, dir: &str) -> crate::app::SessionInfo {
    crate::app::SessionInfo {
        id: id.into(),
        title: format!("title-{id}"),
        parent_id: String::new(),
        directory: dir.into(),
        time: crate::app::SessionTime {
            created: 1,
            updated: 2,
        },
    }
}

// Serialize + redirect config file access to a throwaway temp dir so
// `Config::load()/save()` never touch the real `~/.config/opman`.
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as CFG_LOCK;

struct CfgGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev: Option<std::ffi::OsString>,
    _tmp: tempfile::TempDir,
}

impl CfgGuard {
    fn new() -> Self {
        let lock = CFG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        Self {
            _lock: lock,
            prev,
            _tmp: tmp,
        }
    }
}

impl Drop for CfgGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
}

// ── broadcast_toast / set_theme ─────────────────────────────────────

#[tokio::test]
async fn broadcast_toast_emits_event() {
    let h = WebStateHandle::new_test();
    let mut rx = h.subscribe_events();
    h.broadcast_toast("hello".into(), "info");
    match rx.try_recv() {
        Ok(WebEvent::Toast { message, level }) => {
            assert_eq!(message, "hello");
            assert_eq!(level, "info");
        }
        other => panic!("expected Toast, got {other:?}"),
    }
}

#[tokio::test]
async fn set_theme_stores_and_broadcasts() {
    let h = WebStateHandle::new_test();
    let mut rx = h.subscribe_events();
    h.set_theme(theme()).await;
    assert!(h.get_theme().await.is_some());
    assert!(matches!(rx.try_recv(), Ok(WebEvent::ThemeChanged(_))));
}

// ── switch_project ──────────────────────────────────────────────────

#[tokio::test]
async fn switch_project_valid_and_invalid() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    assert!(h.switch_project(1).await);
    assert_eq!(h.active_project_index().await, 1);
    // Out of range
    assert!(!h.switch_project(9).await);
    assert_eq!(h.active_project_index().await, 1);
}

// ── add_project ─────────────────────────────────────────────────────

#[tokio::test]
async fn add_project_invalid_path() {
    let h = WebStateHandle::new_test();
    let err = h
        .add_project("/definitely/not/here/xyz123", None)
        .await
        .unwrap_err();
    assert!(err.starts_with("Invalid path"), "got: {err}");
}

#[tokio::test]
async fn add_project_path_not_a_directory() {
    let h = WebStateHandle::new_test();
    let file = tempfile::NamedTempFile::new().expect("tmp file");
    let err = h
        .add_project(file.path().to_str().unwrap(), None)
        .await
        .unwrap_err();
    assert_eq!(err, "Path is not a directory");
}

#[tokio::test]
async fn add_project_duplicate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let canonical = std::fs::canonicalize(dir.path()).unwrap();
    let h = WebStateHandle::new_test_with_projects(vec![("existing".into(), canonical.clone())]);
    let err = h
        .add_project(dir.path().to_str().unwrap(), None)
        .await
        .unwrap_err();
    assert_eq!(err, "Project already exists");
}

#[tokio::test]
async fn add_project_success_with_explicit_name() {
    let _g = CfgGuard::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let h = WebStateHandle::new_test();
    let mut rx = h.subscribe_events();
    let (idx, name) = h
        .add_project(dir.path().to_str().unwrap(), Some("MyProj"))
        .await
        .expect("added");
    assert_eq!(idx, 0);
    assert_eq!(name, "MyProj");
    assert!(matches!(rx.try_recv(), Ok(WebEvent::StateChanged)));
}

#[tokio::test]
async fn add_project_derives_name_when_blank() {
    let _g = CfgGuard::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let h = WebStateHandle::new_test();
    // Whitespace-only name falls through to directory-basename derivation.
    let (_idx, name) = h
        .add_project(dir.path().to_str().unwrap(), Some("   "))
        .await
        .expect("added");
    assert!(!name.is_empty());
    // None name also derives from the basename.
    let dir2 = tempfile::tempdir().expect("tempdir2");
    let (_i2, name2) = h
        .add_project(dir2.path().to_str().unwrap(), None)
        .await
        .expect("added2");
    assert!(!name2.is_empty());
}

// ── remove_project ──────────────────────────────────────────────────

#[tokio::test]
async fn remove_project_invalid_index() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    let _g = CfgGuard::new();
    assert_eq!(
        h.remove_project(9).await.unwrap_err(),
        "Invalid project index"
    );
}

#[tokio::test]
async fn remove_project_last_forbidden() {
    let h = WebStateHandle::new_test_with_projects(vec![("only".into(), PathBuf::from("/only"))]);
    let _g = CfgGuard::new();
    assert_eq!(
        h.remove_project(0).await.unwrap_err(),
        "Cannot remove the last project"
    );
}

#[tokio::test]
async fn remove_project_success_adjusts_active() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    let _g = CfgGuard::new();
    // Seed the (temp) config file with two projects so the config-save branch
    // of remove_project (index < config.projects.len()) is exercised.
    {
        let mut cfg = crate::config::Config::load().expect("load default cfg");
        cfg.projects.push(crate::config::ProjectEntry {
            name: "a".into(),
            path: "/a".into(),
            terminal_command: None,
        });
        cfg.projects.push(crate::config::ProjectEntry {
            name: "b".into(),
            path: "/b".into(),
            terminal_command: None,
        });
        cfg.save().expect("seed cfg");
    }
    assert!(h.switch_project(1).await);
    let mut rx = h.subscribe_events();
    let _ = rx.try_recv(); // drain StateChanged from switch_project
    h.remove_project(1).await.expect("removed");
    // active_project was 1, now clamped to len-1 == 0
    assert_eq!(h.active_project_index().await, 0);
    assert_eq!(h.all_project_paths().await.len(), 1);
}

// ── select_session ──────────────────────────────────────────────────

#[tokio::test]
async fn select_session_invalid_project() {
    let h = WebStateHandle::new_test();
    assert!(!h.select_session(5, "sid".into()).await);
}

#[tokio::test]
async fn select_session_missing_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    assert!(!h.select_session(0, "nope".into()).await);
}

#[tokio::test]
async fn select_session_success_clears_unseen() {
    ensure_base_url();
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    h.add_and_activate_session(0, sess("s1", "/a")).await;
    // Mark the session unseen so the clear branch runs.
    h.inner.write().await.unseen_sessions.insert("s1".into(), 3);
    let mut rx = h.subscribe_events();
    assert!(h.select_session(0, "s1".into()).await);
    // Unseen cleared.
    assert!(!h.inner.read().await.unseen_sessions.contains_key("s1"));
    // At least SessionSeen + StateChanged were emitted.
    let mut saw_seen = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, WebEvent::SessionSeen { .. }) {
            saw_seen = true;
        }
    }
    assert!(saw_seen);
}

// ── mark_session_seen ───────────────────────────────────────────────

#[tokio::test]
async fn mark_session_seen_variants() {
    let h = WebStateHandle::new_test();
    // Not present → false
    assert!(!h.mark_session_seen("ghost").await);
    // Present → true + event
    h.inner.write().await.unseen_sessions.insert("s1".into(), 1);
    let mut rx = h.subscribe_events();
    assert!(h.mark_session_seen("s1").await);
    assert!(matches!(rx.try_recv(), Ok(WebEvent::SessionSeen { .. })));
}

// ── add_and_activate_session ────────────────────────────────────────

#[tokio::test]
async fn add_and_activate_session_new_and_duplicate() {
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    h.add_and_activate_session(0, sess("s1", "/a")).await;
    // Duplicate id is not pushed twice but still activates.
    h.add_and_activate_session(0, sess("s1", "/a")).await;
    let (_p, _n, sessions) = h.get_project_sessions(0).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(h.active_session_id().await.as_deref(), Some("s1"));
}

#[tokio::test]
async fn add_and_activate_session_invalid_project_noop() {
    let h = WebStateHandle::new_test();
    // No project at index 0 → silently does nothing (no panic).
    h.add_and_activate_session(0, sess("s1", "/a")).await;
    assert!(h.active_session_id().await.is_none());
}

// ── toggle_panel ────────────────────────────────────────────────────

#[tokio::test]
async fn toggle_panel_all_variants_and_invalid() {
    let h = WebStateHandle::new_test();
    for p in [
        "Sidebar",
        "terminal_pane",
        "NeovimPane",
        "integrated_terminal",
        "GitPanel",
    ] {
        assert!(h.toggle_panel(p).await, "{p} should toggle");
    }
    assert!(!h.toggle_panel("Bogus").await);

    // Verify a toggle actually flipped state via get_state.
    let before = h.get_state().await.panels.sidebar;
    h.toggle_panel("sidebar").await;
    assert_ne!(before, h.get_state().await.panels.sidebar);
}

// ── focus_panel ─────────────────────────────────────────────────────

#[tokio::test]
async fn focus_panel_normalizes_and_rejects() {
    let h = WebStateHandle::new_test();
    assert!(h.focus_panel("sidebar").await);
    assert_eq!(h.get_state().await.focused, "Sidebar");
    assert!(h.focus_panel("terminal_pane").await);
    assert_eq!(h.get_state().await.focused, "TerminalPane");
    assert!(h.focus_panel("neovim_pane").await);
    assert_eq!(h.get_state().await.focused, "NeovimPane");
    assert!(h.focus_panel("integrated_terminal").await);
    assert_eq!(h.get_state().await.focused, "IntegratedTerminal");
    assert!(h.focus_panel("git_panel").await);
    assert_eq!(h.get_state().await.focused, "GitPanel");
    // Already PascalCase passes through the `other` arm.
    assert!(h.focus_panel("Sidebar").await);
    assert_eq!(h.get_state().await.focused, "Sidebar");
    // Invalid name rejected.
    assert!(!h.focus_panel("Nope").await);
}

/// A session's runner label must survive later events.
///
/// A runner engine announces `session.created` while the creating request is
/// still in flight, so the announcement can arrive after the explicit label.
/// Overwriting it would report the wrong owner and make the session's next turn
/// look like a runner switch.
#[tokio::test]
async fn session_runner_label_is_set_once_and_never_downgraded() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    h.add_and_activate_session(0, sess("s1", "/proj")).await;

    h.set_session_runner_if_absent("s1", "claude").await;
    let runner_of = |state: &WebAppState| {
        state.projects[0]
            .sessions
            .iter()
            .find(|session| session.id == "s1")
            .map(|session| session.runner.clone())
    };
    assert_eq!(runner_of(&h.get_state().await).as_deref(), Some("claude"));

    // A late announcement from another stream must not steal ownership.
    h.set_session_runner_if_absent("s1", "claude-code").await;
    assert_eq!(runner_of(&h.get_state().await).as_deref(), Some("claude"));

    // An explicit label (creation, handoff) still wins.
    h.set_session_runner("s1", "claude-code").await;
    assert_eq!(
        runner_of(&h.get_state().await).as_deref(),
        Some("claude-code")
    );
}

/// `session_runner` reports only what is known: callers must be able to tell
/// "owned by the default runner" from "owner unknown", which the `/api/state`
/// projection cannot express because it falls back to the default.
#[tokio::test]
async fn session_runner_query_does_not_invent_a_default() {
    let h = WebStateHandle::new_test();
    assert_eq!(h.session_runner("unknown").await, None);
    h.set_session_runner("s1", "codex").await;
    assert_eq!(h.session_runner("s1").await.as_deref(), Some("codex"));
    // The snapshot still labels unknown sessions with the process default.
    h.set_default_runner("claude-code").await;
    assert_eq!(h.get_state().await.default_runner, "claude-code");
}
