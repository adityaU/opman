use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use crate::app::{App, BackgroundEvent, WatcherConfig, WatcherField, WatcherModalState, WatcherSessionEntry};
use super::watcher_keys;

/// Build the session list and open the watcher modal.
pub(super) fn open_watcher_modal(app: &mut App) {
    let mut sessions = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Current session in the current PTY
    if let Some(project) = app.projects.get(app.active_project) {
        if let Some(ref sid) = project.active_session {
            if let Some(s) = project.sessions.iter().find(|s| &s.id == sid) {
                seen.insert(sid.clone());
                sessions.push(WatcherSessionEntry {
                    session_id: sid.clone(),
                    title: if s.title.is_empty() {
                        sid[..8.min(sid.len())].to_string()
                    } else {
                        s.title.clone()
                    },
                    project_name: project.name.clone(),
                    project_idx: app.active_project,
                    is_current: true,
                    is_active: app.active_sessions.contains(sid),
                    has_watcher: app.session_watchers.contains_key(sid),
                });
            }
        }
    }

    // 2. All running/active sessions
    for sid in &app.active_sessions {
        if seen.contains(sid) {
            continue;
        }
        // Find the project/session info
        let project_idx = app.session_ownership.get(sid).copied();
        if let Some(pidx) = project_idx {
            if let Some(project) = app.projects.get(pidx) {
                if let Some(s) = project.sessions.iter().find(|s| s.id == *sid) {
                    seen.insert(sid.clone());
                    sessions.push(WatcherSessionEntry {
                        session_id: sid.clone(),
                        title: if s.title.is_empty() {
                            sid[..8.min(sid.len())].to_string()
                        } else {
                            s.title.clone()
                        },
                        project_name: project.name.clone(),
                        project_idx: pidx,
                        is_current: false,
                        is_active: true,
                        has_watcher: app.session_watchers.contains_key(sid),
                    });
                }
            }
        }
    }

    // 3. Sessions that already have watchers
    for (sid, watcher) in &app.session_watchers {
        if seen.contains(sid) {
            continue;
        }
        let project_name = app
            .projects
            .get(watcher.project_idx)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "?".into());
        let title = app
            .projects
            .get(watcher.project_idx)
            .and_then(|p| p.sessions.iter().find(|s| &s.id == sid))
            .map(|s| {
                if s.title.is_empty() {
                    sid[..8.min(sid.len())].to_string()
                } else {
                    s.title.clone()
                }
            })
            .unwrap_or_else(|| sid[..8.min(sid.len())].to_string());
        seen.insert(sid.clone());
        sessions.push(WatcherSessionEntry {
            session_id: sid.clone(),
            title,
            project_name,
            project_idx: watcher.project_idx,
            is_current: false,
            is_active: app.active_sessions.contains(sid),
            has_watcher: true,
        });
    }

    // 4. Add remaining sessions from all projects (non-active, non-watched)
    for (pidx, project) in app.projects.iter().enumerate() {
        for s in &project.sessions {
            if seen.contains(&s.id) {
                continue;
            }
            seen.insert(s.id.clone());
            sessions.push(WatcherSessionEntry {
                session_id: s.id.clone(),
                title: if s.title.is_empty() {
                    s.id[..8.min(s.id.len())].to_string()
                } else {
                    s.title.clone()
                },
                project_name: project.name.clone(),
                project_idx: pidx,
                is_current: false,
                is_active: app.active_sessions.contains(&s.id),
                has_watcher: false,
            });
        }
    }

    app.watcher_modal = Some(WatcherModalState {
        sessions,
        selected_session_idx: 0,
        session_scroll: 0,
        active_field: WatcherField::SessionList,
        message_lines: vec!["Continue if you have next steps, or stop and ask for clarification if you are unsure how to proceed.".to_string()],
        message_cursor_row: 0,
        message_cursor_col: 0,
        include_original: false,
        session_messages: Vec::new(),
        selected_message_idx: 0,
        message_scroll: 0,
        idle_timeout_secs: 10,
        timeout_input: "10".to_string(),
        hang_message_lines: vec!["The previous attempt appears to have hung. Please retry the last step.".to_string()],
        hang_message_cursor_row: 0,
        hang_message_cursor_col: 0,
        hang_timeout_secs: 180,
        hang_timeout_input: "180".to_string(),
    });

    // Fetch messages for the first selected session
    if let Some(ref modal) = app.watcher_modal {
        if let Some(entry) = modal.sessions.first() {
            fetch_watcher_session_messages(app, &entry.session_id, entry.project_idx);
        }
    }
}

/// Fetch user messages for a session and send them via background event.
pub(super) fn fetch_watcher_session_messages(app: &App, session_id: &str, project_idx: usize) {
    let sid = session_id.to_string();
    let base_url = crate::app::base_url().to_string();
    let proj_dir = app
        .projects
        .get(project_idx)
        .map(|p| p.path.to_string_lossy().to_string())
        .unwrap_or_default();
    let bg_tx = app.bg_tx.clone();
    tokio::spawn(async move {
        let client = crate::api::ApiClient::new();
        match client
            .fetch_session_messages(&base_url, &proj_dir, &sid)
            .await
        {
            Ok(messages) => {
                let _ = bg_tx.send(BackgroundEvent::WatcherSessionMessages {
                    session_id: sid,
                    messages,
                });
            }
            Err(e) => {
                tracing::warn!("Failed to fetch session messages for watcher: {e}");
            }
        }
    });
}

pub(super) fn handle_watcher_modal_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    let active_field = match &app.watcher_modal {
        Some(m) => m.active_field,
        None => return Ok(()),
    };

    // Global keys
    match key.code {
        KeyCode::Esc => {
            app.watcher_modal = None;
            return Ok(());
        }
        KeyCode::Tab => {
            if let Some(ref mut m) = app.watcher_modal {
                m.next_field();
            }
            return Ok(());
        }
        KeyCode::BackTab => {
            if let Some(ref mut m) = app.watcher_modal {
                m.prev_field();
            }
            return Ok(());
        }
        _ => {}
    }

    // Field-specific keys
    match active_field {
        WatcherField::SessionList => {
            watcher_keys::handle_watcher_session_list_keys(app, key)?;
        }
        WatcherField::Message => {
            watcher_keys::handle_watcher_message_keys(app, key)?;
        }
        WatcherField::IncludeOriginal => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                if let Some(ref mut m) = app.watcher_modal {
                    m.include_original = !m.include_original;
                }
            }
        }
        WatcherField::OriginalMessageList => {
            watcher_keys::handle_watcher_orig_message_keys(app, key)?;
        }
        WatcherField::TimeoutInput => {
            watcher_keys::handle_watcher_timeout_keys(app, key)?;
        }
        WatcherField::HangMessage => {
            watcher_keys::handle_watcher_hang_message_keys(app, key)?;
        }
        WatcherField::HangTimeoutInput => {
            watcher_keys::handle_watcher_hang_timeout_keys(app, key)?;
        }
    }
    Ok(())
}

pub(super) fn submit_watcher(app: &mut App) {
    let modal = match app.watcher_modal.take() {
        Some(m) => m,
        None => return,
    };

    let entry = match modal.sessions.get(modal.selected_session_idx) {
        Some(e) => e,
        None => return,
    };

    let msg = modal.message_text();
    if msg.trim().is_empty() {
        app.toast_message = Some((
            "Watcher not added: continuation message is empty".into(),
            std::time::Instant::now(),
        ));
        return;
    }

    let original_message = if modal.include_original {
        modal
            .session_messages
            .get(modal.selected_message_idx)
            .map(|m| m.text.clone())
    } else {
        None
    };

    let session_id = entry.session_id.clone();
    let project_idx = entry.project_idx;
    let timeout_secs = modal.idle_timeout_secs.max(1);

    let config = WatcherConfig {
        session_id: session_id.clone(),
        project_idx,
        idle_timeout_secs: timeout_secs,
        continuation_message: msg.clone(),
        include_original: modal.include_original,
        original_message: original_message.clone(),
        hang_message: modal.hang_message_text(),
        hang_timeout_secs: modal.hang_timeout_secs.max(30),
    };

    app.session_watchers.insert(session_id.clone(), config);
    app.toast_message = Some((
        format!("Watcher added for session ({}s timeout)", timeout_secs),
        std::time::Instant::now(),
    ));

    // If the session is NOT currently active (already idle), trigger the watcher
    // immediately (but still respect active children).
    if !app.active_sessions.contains(&session_id) {
        let has_active_children = app
            .session_children
            .get(&session_id)
            .map(|children| children.iter().any(|cid| app.active_sessions.contains(cid)))
            .unwrap_or(false);
        app.try_trigger_watcher(&session_id, has_active_children);
    } else {
        tracing::info!(
            session_id = %session_id,
            "Watcher: session currently active, will trigger on next idle event"
        );
    }
}
