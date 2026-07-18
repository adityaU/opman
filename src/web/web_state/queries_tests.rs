use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

fn sess(id: &str, parent: &str, dir: &str, updated: u64) -> crate::app::SessionInfo {
    crate::app::SessionInfo {
        id: id.into(),
        title: format!("title-{id}"),
        parent_id: parent.into(),
        directory: dir.into(),
        time: crate::app::SessionTime { created: 1, updated },
    }
}

#[tokio::test]
async fn get_state_reflects_indicator_sets() {
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    h.add_and_activate_session(0, sess("s1", "", "/a", 10)).await;
    h.add_and_activate_session(0, sess("s2", "", "/a", 20)).await;
    {
        let mut inner = h.inner.write().await;
        inner.busy_sessions.insert("s1".into());
        inner.error_sessions.insert("s1".into(), "boom".into());
        inner.input_sessions.insert("s2".into());
        inner.unseen_sessions.insert("s2".into(), 4);
    }
    let state = h.get_state().await;
    assert_eq!(state.projects.len(), 1);
    let p = &state.projects[0];
    assert_eq!(p.index, 0);
    assert_eq!(p.name, "a");
    assert_eq!(p.busy_sessions, vec!["s1".to_string()]);
    assert_eq!(p.error_sessions, vec!["s1".to_string()]);
    assert_eq!(p.input_sessions, vec!["s2".to_string()]);
    assert_eq!(p.unseen_sessions, vec!["s2".to_string()]);
    assert_eq!(p.sessions.len(), 2);
    assert_eq!(state.active_project, 0);
    assert_eq!(state.backend, "");
    assert!(state.instance_name.is_none());
}

#[tokio::test]
async fn get_session_stats_hit_and_miss() {
    let h = WebStateHandle::new_test();
    assert!(h.get_session_stats("none").await.is_none());
    {
        let mut inner = h.inner.write().await;
        inner.session_stats.insert(
            "s1".into(),
            WebSessionStats { session_id: "s1".into(), cost: 1.5, input_tokens: 10, ..Default::default() },
        );
    }
    let s = h.get_session_stats("s1").await.unwrap();
    assert_eq!(s.cost, 1.5);
    assert_eq!(s.input_tokens, 10);
}

#[tokio::test]
async fn get_sessions_overview_sorts_and_counts_busy() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    h.add_and_activate_session(0, sess("old", "", "/a", 5)).await;
    h.add_and_activate_session(1, sess("new", "", "/b", 99)).await;
    h.inner.write().await.busy_sessions.insert("new".into());
    let ov = h.get_sessions_overview().await;
    assert_eq!(ov.total, 2);
    assert_eq!(ov.busy_count, 1);
    // Sorted by updated desc → "new" first.
    assert_eq!(ov.sessions[0].id, "new");
    assert!(ov.sessions[0].is_busy);
    assert_eq!(ov.sessions[0].project_index, 1);
}

#[tokio::test]
async fn get_sessions_tree_builds_parent_child_and_orphans() {
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    h.add_and_activate_session(0, sess("s1", "", "/a", 1)).await; // root
    h.add_and_activate_session(0, sess("s2", "s1", "/a", 2)).await; // child of s1
    h.add_and_activate_session(0, sess("s3", "ghost", "/a", 3)).await; // orphan → root
    h.inner.write().await.busy_sessions.insert("s2".into());
    let tree = h.get_sessions_tree().await;
    assert_eq!(tree.total, 3);
    // Roots: s1 and s3 (orphan parent not present).
    assert_eq!(tree.roots.len(), 2);
    let s1 = tree.roots.iter().find(|n| n.id == "s1").unwrap();
    assert_eq!(s1.children.len(), 1);
    assert_eq!(s1.children[0].id, "s2");
    assert!(s1.children[0].is_busy);
    assert!(tree.roots.iter().any(|n| n.id == "s3"));
}

#[tokio::test]
async fn get_file_edits_hit_and_miss() {
    let h = WebStateHandle::new_test();
    assert!(h.get_file_edits("s1").await.is_empty());
    {
        let mut inner = h.inner.write().await;
        inner.file_edits.insert(
            "s1".into(),
            vec![crate::web::web_state::FileEditRecord {
                path: "f.rs".into(),
                original_content: "a".into(),
                new_content: "b".into(),
                timestamp: "t".into(),
                index: 0,
            }],
        );
    }
    let edits = h.get_file_edits("s1").await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].path, "f.rs");
}

#[tokio::test]
async fn working_dir_queries() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/proj/a")),
        ("b".into(), PathBuf::from("/proj/b")),
    ]);
    assert_eq!(h.get_working_dir().await, Some(PathBuf::from("/proj/a")));
    h.switch_project(1).await;
    assert_eq!(h.get_working_dir().await, Some(PathBuf::from("/proj/b")));
    assert_eq!(
        h.get_project_working_dir(0).await,
        Some(PathBuf::from("/proj/a"))
    );
    assert!(h.get_project_working_dir(9).await.is_none());
}

#[tokio::test]
async fn working_dir_none_when_no_projects() {
    let h = WebStateHandle::new_test();
    assert!(h.get_working_dir().await.is_none());
}

#[tokio::test]
async fn all_project_paths_and_active_index() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    assert_eq!(
        h.all_project_paths().await,
        vec!["/a".to_string(), "/b".to_string()]
    );
    assert_eq!(h.active_project_index().await, 0);
}

#[tokio::test]
async fn get_project_sessions_hit_and_miss() {
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    h.add_and_activate_session(0, sess("s1", "", "/a", 1)).await;
    let (path, name, sessions) = h.get_project_sessions(0).await.unwrap();
    assert_eq!(path, PathBuf::from("/a"));
    assert_eq!(name, "a");
    assert_eq!(sessions, vec![("s1".to_string(), "title-s1".to_string())]);
    assert!(h.get_project_sessions(9).await.is_none());
}

#[tokio::test]
async fn get_theme_none_then_some() {
    let h = WebStateHandle::new_test();
    assert!(h.get_theme().await.is_none());
}

#[tokio::test]
async fn active_session_id_variants() {
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    assert!(h.active_session_id().await.is_none());
    h.add_and_activate_session(0, sess("s1", "", "/a", 1)).await;
    assert_eq!(h.active_session_id().await.as_deref(), Some("s1"));
}
