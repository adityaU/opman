//! Handles BackgroundEvent dispatch for the main App event loop.
//! Larger SSE arms are delegated to helpers in `background_sse.rs` / `background_sse_slack.rs`.

use std::path::PathBuf;
use tracing::{debug, info};

use crate::app::{
    diff_snapshot_lines, App, BackgroundEvent, ModelLimits, SessionInfo, SessionResources,
};
use crate::theme::color_to_hex;
use crate::theme::ThemeColors;

impl App {
    pub fn handle_background_event(&mut self, event: BackgroundEvent) {
        match event {
            BackgroundEvent::PtySpawned {
                project_idx,
                session_id,
                pty,
            } => {
                if let Some(project) = self.projects.get_mut(project_idx) {
                    info!(name = %project.name, session_id, "PTY spawned via background event");
                    project.ptys.insert(session_id.clone(), pty);
                    project.active_session = Some(session_id);
                }
                self.resize_all_ptys();
            }
            BackgroundEvent::SessionsFetched {
                project_idx,
                sessions,
            } => {
                if let Some(project) = self.projects.get_mut(project_idx) {
                    let dir = project.path.to_string_lossy().to_string();
                    let filtered: Vec<SessionInfo> = sessions
                        .into_iter()
                        .filter(|s| s.directory == dir)
                        .collect();
                    for s in &filtered {
                        self.session_ownership.insert(s.id.clone(), project_idx);
                    }
                    project.sessions = filtered;
                }
            }
            BackgroundEvent::SessionFetchFailed { project_idx } => {
                debug!(project_idx, "Session fetch failed (non-fatal)");
            }
            BackgroundEvent::SessionSelected {
                project_idx,
                session_id,
            } => {
                debug!(
                    project_idx,
                    session_id, "Session selected via background event"
                );
            }
            BackgroundEvent::ProjectActivated { project_idx } => {
                debug!(project_idx, "Project fully activated via background event");
            }
            BackgroundEvent::SessionStatusFetched { busy_sessions } => {
                tracing::info!(
                    count = busy_sessions.len(),
                    "SessionStatusFetched: bootstrapping active_sessions"
                );
                for sid in busy_sessions {
                    self.active_sessions.insert(sid);
                }
            }
            BackgroundEvent::SseSessionCreated {
                project_idx,
                session,
            } => {
                self.handle_sse_session_created(project_idx, session);
            }
            BackgroundEvent::SseSessionUpdated {
                project_idx,
                session,
            } => {
                if let Some(&owner) = self.session_ownership.get(&session.id) {
                    if owner != project_idx {
                        return;
                    }
                }
                self.active_sessions.insert(session.id.clone());
                if let Some(project) = self.projects.get_mut(project_idx) {
                    if let Some(existing) = project.sessions.iter_mut().find(|s| s.id == session.id)
                    {
                        *existing = session;
                    }
                }
            }
            BackgroundEvent::SseSessionDeleted {
                project_idx,
                session_id,
            } => {
                self.handle_sse_session_deleted(project_idx, session_id);
            }
            BackgroundEvent::SseSessionIdle {
                session_id,
                project_idx,
            } => {
                self.handle_sse_session_idle(session_id, project_idx);
            }
            BackgroundEvent::SseSessionBusy { session_id } => {
                tracing::debug!(session_id = %session_id, has_pending = self.watcher_pending.contains_key(&session_id), "SseSessionBusy received");
                self.active_sessions.insert(session_id.clone());
                self.watcher_idle_since.remove(&session_id);
                if let Some(abort_handle) = self.watcher_pending.remove(&session_id) {
                    tracing::info!(session_id = %session_id, "Watcher: cancelled pending timer (session busy)");
                    abort_handle.abort();
                }
            }
            BackgroundEvent::SseFileEdited {
                project_idx,
                file_path,
            } => {
                self.handle_sse_file_edited(project_idx, file_path);
            }
            BackgroundEvent::TodosFetched { session_id, todos } => {
                debug!(session_id, count = todos.len(), "Todos fetched");
                if let Some(ref mut panel) = self.todo_panel {
                    if panel.session_id == session_id {
                        panel.todos = todos;
                        if panel.selected >= panel.todos.len() {
                            panel.selected = panel.todos.len().saturating_sub(1);
                        }
                    }
                }
            }
            BackgroundEvent::SseTodoUpdated { session_id, todos } => {
                self.handle_sse_todo_updated(session_id, todos);
            }
            BackgroundEvent::SseMessageUpdated {
                session_id,
                cost,
                input_tokens,
                output_tokens,
                reasoning_tokens,
                cache_read,
                cache_write,
            } => {
                self.handle_sse_message_updated(
                    session_id,
                    cost,
                    input_tokens,
                    output_tokens,
                    reasoning_tokens,
                    cache_read,
                    cache_write,
                );
            }
            BackgroundEvent::SsePermissionAsked {
                project_idx,
                request,
            } => {
                self.handle_sse_permission_asked(project_idx, request);
            }
            BackgroundEvent::SseQuestionAsked {
                project_idx,
                request,
            } => {
                self.handle_sse_question_asked(project_idx, request);
            }
            BackgroundEvent::ModelLimitsFetched {
                project_idx,
                context_window,
            } => {
                self.model_limits
                    .insert(project_idx, ModelLimits { context_window });
                debug!(project_idx, context_window, "Model context window fetched");
            }
            BackgroundEvent::McpSocketRequest {
                project_idx,
                session_id,
                pending,
            } => {
                let resolved_sid = if session_id.is_empty() {
                    self.projects
                        .get(project_idx)
                        .and_then(|p| p.active_session.clone())
                        .unwrap_or_default()
                } else {
                    session_id
                };
                let response =
                    self.handle_mcp_request(project_idx, &resolved_sid, &pending.request);
                let _ = pending.reply_tx.send(response);
            }
            BackgroundEvent::WatcherSessionMessages {
                session_id,
                messages,
            } => {
                if let Some(ref mut modal) = self.watcher_modal {
                    if let Some(entry) = modal.sessions.get(modal.selected_session_idx) {
                        if entry.session_id == session_id {
                            modal.session_messages = messages;
                            modal.selected_message_idx = 0;
                            modal.message_scroll = 0;
                        }
                    }
                }
            }
            BackgroundEvent::SlackEvent(slack_event) => {
                self.handle_slack_event(slack_event);
            }
            BackgroundEvent::RoutinesFetched { routines } => {
                debug!(count = routines.len(), "Routines fetched");
                if let Some(ref mut panel) = self.routine_panel {
                    panel.routines = routines;
                    panel.loading = false;
                    if panel.selected >= panel.routines.len() {
                        panel.selected = panel.routines.len().saturating_sub(1);
                    }
                }
            }
            BackgroundEvent::RoutineRunCompleted {
                routine_id,
                success,
                message,
            } => {
                debug!(routine_id, success, message, "Routine run completed");
                if let Some(ref mut panel) = self.routine_panel {
                    if panel.running.as_deref() == Some(&routine_id) {
                        panel.running = None;
                    }
                }
                let status = if success { "done" } else { "failed" };
                self.toast_message = Some((
                    format!("Routine {}: {}", status, message),
                    std::time::Instant::now(),
                ));
            }
            BackgroundEvent::RoutineCreated { routine } => {
                debug!(id = %routine.id, name = %routine.name, "Routine created");
                if let Some(ref mut panel) = self.routine_panel {
                    panel.routines.push(routine.clone());
                    panel.editing = None;
                }
                self.toast_message = Some((
                    format!("Routine created: {}", routine.name),
                    std::time::Instant::now(),
                ));
            }
            BackgroundEvent::RoutineDeleted {
                routine_id,
                success,
            } => {
                debug!(routine_id, success, "Routine deleted");
                if let Some(ref mut panel) = self.routine_panel {
                    if success {
                        let name = panel
                            .routines
                            .iter()
                            .find(|r| r.id == routine_id)
                            .map(|r| r.name.clone())
                            .unwrap_or_default();
                        panel.routines.retain(|r| r.id != routine_id);
                        if panel.selected >= panel.routines.len() {
                            panel.selected = panel.routines.len().saturating_sub(1);
                        }
                        self.toast_message = Some((
                            format!("Routine deleted: {}", name),
                            std::time::Instant::now(),
                        ));
                    } else {
                        self.toast_message = Some((
                            "Failed to delete routine".to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                    panel.confirm_delete = None;
                }
            }
        }
    }

    /// Build the neovim command sequence for following a file edit (used by background_sse.rs).
    pub(crate) fn build_neovim_edit_cmds(
        file_path: &str,
        project_path: &PathBuf,
        resources: &mut SessionResources,
        theme: &ThemeColors,
    ) -> Vec<String> {
        let abs_path = if std::path::Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            project_path.join(file_path).to_string_lossy().to_string()
        };
        let vim_str_path = abs_path.replace('\'', "''");
        let mut cmds = vec![format!(
            "\x1b:execute 'edit! ' . fnameescape('{}')\r",
            vim_str_path
        )];

        let current_content = std::fs::read_to_string(&abs_path).unwrap_or_default();
        let old_content = if let Some(snap) = resources.file_snapshots.get(&abs_path) {
            snap.clone()
        } else {
            let rel_path = std::path::Path::new(&abs_path)
                .strip_prefix(project_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string());
            let git_show = std::process::Command::new("git")
                .args(["show", &format!("HEAD:{}", rel_path)])
                .current_dir(project_path)
                .output();
            match git_show {
                Ok(output) if output.status.success() => {
                    String::from_utf8_lossy(&output.stdout).to_string()
                }
                _ => String::new(),
            }
        };

        let (added, deleted) = if old_content.is_empty() && !current_content.is_empty() {
            let line_count = current_content.lines().count().max(1);
            ((1..=line_count).collect::<Vec<_>>(), Vec::new())
        } else {
            diff_snapshot_lines(&old_content, &current_content)
        };
        resources
            .file_snapshots
            .insert(abs_path.clone(), current_content);
        debug!(
            added_count = added.len(),
            deleted_count = deleted.len(),
            "SseFileEdited: snapshot diff computed"
        );

        if !added.is_empty() || !deleted.is_empty() {
            let success_hex = color_to_hex(theme.success);
            let error_hex = color_to_hex(theme.error);
            cmds.push(format!(
                "\x1b:highlight DiffAddLine guibg={} guifg=black\r",
                success_hex
            ));
            cmds.push(format!(
                "\x1b:highlight DiffDelLine guibg={} guifg=black\r",
                error_hex
            ));
            cmds.push("\x1b:sign define diff_add text=+ texthl=DiffAddLine\r".to_string());
            cmds.push("\x1b:sign define diff_del text=- texthl=DiffDelLine\r".to_string());
            cmds.push("\x1b:execute 'sign unplace * buffer=' . bufnr('%')\r".to_string());
            let mut sign_id = 1;
            let mut first_line: Option<usize> = None;
            for line in &added {
                first_line = Some(first_line.map_or(*line, |m: usize| m.min(*line)));
                cmds.push(format!(
                    "\x1b:execute 'sign place {} line={} name=diff_add buffer=' . bufnr('%')\r",
                    sign_id, line
                ));
                sign_id += 1;
            }
            for line in &deleted {
                first_line = Some(first_line.map_or(*line, |m: usize| m.min(*line)));
                cmds.push(format!(
                    "\x1b:execute 'sign place {} line={} name=diff_del buffer=' . bufnr('%')\r",
                    sign_id, line
                ));
                sign_id += 1;
            }
            if let Some(l) = first_line {
                cmds.push(format!("\x1b:call cursor({}, 0)\r", l));
                cmds.push("\x1b:normal! zz\r".to_string());
            }
        }
        cmds
    }
}
