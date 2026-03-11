//! SSE session lifecycle helpers for `App::handle_background_event`.
//!
//! Contains: `handle_sse_session_created`, `handle_sse_session_deleted`,
//! `handle_sse_session_idle`, and `handle_sse_file_edited`.

use tracing::{debug, info};

use crate::app::{App, SessionInfo};

impl App {
    /// Handle `BackgroundEvent::SseSessionCreated`.
    pub(crate) fn handle_sse_session_created(
        &mut self,
        project_idx: usize,
        session: SessionInfo,
    ) {
        let awaiting = self.awaiting_new_session == Some(project_idx);

        if !awaiting {
            if let Some(&owner) = self.session_ownership.get(&session.id) {
                if owner != project_idx {
                    return;
                }
            }
        }

        // Track parent→child relationship for watcher suppression.
        if !session.parent_id.is_empty() {
            tracing::info!(
                session_id = %session.id,
                parent_id = %session.parent_id,
                "SseSessionCreated: registering child→parent relationship"
            );
            self.session_children
                .entry(session.parent_id.clone())
                .or_default()
                .insert(session.id.clone());

            // Spawn a dedicated Slack thread for the subagent.
            if let Some(ref slack_state) = self.slack_state {
                if let Some(ref auth) = self.slack_auth {
                    let parent_thread_info = {
                        if let Ok(s) = slack_state.try_lock() {
                            s.session_threads.get(&session.parent_id).cloned()
                        } else {
                            None
                        }
                    };

                    if let Some((parent_channel, parent_thread_ts)) = parent_thread_info {
                        let child_sid = session.id.clone();
                        let child_title = session.title.clone();
                        let parent_sid = session.parent_id.clone();
                        let bot_token = auth.bot_token.clone();
                        let base_url = crate::app::base_url().to_string();
                        let project_dir = self
                            .projects
                            .get(project_idx)
                            .map(|p| p.path.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let buffer_secs = self.config.settings.slack.relay_buffer_secs;
                        let st = slack_state.clone();

                        tokio::spawn(async move {
                            Self::setup_subagent_slack_thread(
                                project_idx, child_sid, child_title, parent_sid,
                                bot_token, base_url, project_dir, buffer_secs,
                                parent_channel, parent_thread_ts, st,
                            )
                            .await;
                        });
                    }
                }
            }
        }

        if let Some(project) = self.projects.get_mut(project_idx) {
            if !project.sessions.iter().any(|s| s.id == session.id) {
                info!(
                    name = %project.name,
                    session_id = %session.id,
                    parent_id = %session.parent_id,
                    "SSE: new session created"
                );
                self.active_sessions.insert(session.id.clone());
                self.session_ownership
                    .insert(session.id.clone(), project_idx);
                project.sessions.insert(0, session.clone());
            }
            if awaiting {
                if let Some(pty) = project.ptys.remove("__new__") {
                    project.ptys.insert(session.id.clone(), pty);
                    project.active_session = Some(session.id.clone());
                }
                self.awaiting_new_session = None;
                self.pending_session_select = Some((project_idx, session.id.clone()));
                if !self.pending_slack_messages.is_empty() {
                    self.drain_pending_slack_messages(project_idx, &session.id);
                }
            }
        }
    }

    /// Handle `BackgroundEvent::SseSessionDeleted`.
    pub(crate) fn handle_sse_session_deleted(
        &mut self,
        project_idx: usize,
        session_id: String,
    ) {
        self.active_sessions.remove(&session_id);
        self.session_ownership.remove(&session_id);

        let parent_id = self
            .projects
            .get(project_idx)
            .and_then(|p| p.sessions.iter().find(|s| s.id == session_id))
            .map(|s| s.parent_id.clone())
            .unwrap_or_default();
        if !parent_id.is_empty() {
            if let Some(children) = self.session_children.get_mut(&parent_id) {
                children.remove(&session_id);
                if children.is_empty() {
                    self.session_children.remove(&parent_id);
                }
            }
        }
        self.session_children.remove(&session_id);

        if let Some(project) = self.projects.get_mut(project_idx) {
            project.sessions.retain(|s| s.id != session_id);
            if let Some(mut resources) = project.session_resources.remove(&session_id) {
                for shell_pty in &mut resources.shell_ptys {
                    let _ = shell_pty.kill();
                }
                if let Some(ref mut nvim) = resources.neovim_pty {
                    let _ = nvim.kill();
                }
            }
            if let Some(mut pty) = project.ptys.remove(&session_id) {
                let _ = pty.kill();
            }
            if project.active_session.as_deref() == Some(&session_id) {
                project.active_session = None;
            }
        }
    }

    /// Handle `BackgroundEvent::SseSessionIdle`.
    pub(crate) fn handle_sse_session_idle(
        &mut self,
        session_id: String,
        project_idx: usize,
    ) {
        let has_active_children = self
            .session_children
            .get(&session_id)
            .map(|children| {
                children.iter().any(|cid| self.active_sessions.contains(cid))
            })
            .unwrap_or(false);

        tracing::info!(
            session_id = %session_id,
            has_watcher = self.session_watchers.contains_key(&session_id),
            was_active = self.active_sessions.contains(&session_id),
            has_active_children,
            "SseSessionIdle received"
        );
        self.active_sessions.remove(&session_id);

        if !self.pending_slack_messages.is_empty() {
            self.drain_pending_slack_messages(project_idx, &session_id);
        }
        self.try_trigger_watcher(&session_id, has_active_children);

        let parent_id = self
            .projects
            .iter()
            .flat_map(|p| p.sessions.iter())
            .find(|s| s.id == session_id)
            .map(|s| s.parent_id.clone())
            .unwrap_or_default();
        if !parent_id.is_empty() && !self.active_sessions.contains(&parent_id) {
            let parent_has_active_children = self
                .session_children
                .get(&parent_id)
                .map(|children| {
                    children.iter().any(|cid| self.active_sessions.contains(cid))
                })
                .unwrap_or(false);
            if !parent_has_active_children {
                tracing::info!(
                    parent_id = %parent_id,
                    child_id = %session_id,
                    "SseSessionIdle: last child went idle, re-evaluating parent watcher"
                );
                self.try_trigger_watcher(&parent_id, false);
            }
        }

        if let Some(ss) = self.slack_state.clone() {
            let sid = session_id.clone();
            tokio::spawn(async move {
                let mut guard = ss.lock().await;
                if guard.streaming_messages.contains_key(&sid) {
                    tracing::info!(
                        "SseSessionIdle: requesting stream stop for session {}",
                        &sid[..8.min(sid.len())]
                    );
                    guard.request_stream_stop(&sid);
                }
            });
        }
    }

    /// Handle `BackgroundEvent::SseFileEdited`.
    pub(crate) fn handle_sse_file_edited(&mut self, project_idx: usize, file_path: String) {
        debug!(
            project_idx, file_path,
            follow_enabled = self.config.settings.follow_edits_in_neovim,
            neovim_mcp = self.neovim_mcp_enabled,
            active_project = self.active_project,
            "SseFileEdited received"
        );
        if !self.neovim_mcp_enabled
            && self.config.settings.follow_edits_in_neovim
            && project_idx == self.active_project
        {
            let has_nvim = self
                .projects
                .get(project_idx)
                .and_then(|p| p.active_resources())
                .map(|r| r.neovim_pty.is_some())
                .unwrap_or(false);
            if !has_nvim {
                self.ensure_neovim_pty();
            }
            if let Some(project) = self.projects.get_mut(project_idx) {
                let project_path = project.path.clone();
                if let Some(resources) = project.active_resources_mut() {
                    let has_nvim = resources.neovim_pty.is_some();
                    debug!(has_nvim, "SseFileEdited: project found, checking neovim_pty");
                    if resources.neovim_pty.is_some() {
                        let cmds = Self::build_neovim_edit_cmds(
                            &file_path, &project_path, resources, &self.theme,
                        );
                        let batch = cmds.concat();
                        debug!(cmd_count = cmds.len(), "SseFileEdited: writing batched vim cmds");
                        if let Some(ref mut nvim) = resources.neovim_pty {
                            let _ = nvim.write(batch.as_bytes());
                        }
                    } else {
                        debug!("SseFileEdited: neovim_pty is still None after ensure, skipping");
                    }
                } else {
                    debug!("SseFileEdited: no active session resources");
                }
            } else {
                debug!(project_idx, "SseFileEdited: project not found at index");
            }
        } else {
            debug!(
                follow_enabled = self.config.settings.follow_edits_in_neovim,
                project_match = (project_idx == self.active_project),
                "SseFileEdited: skipped (follow disabled or wrong project)"
            );
        }
    }
}
