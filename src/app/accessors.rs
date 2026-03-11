//! Small App utility methods: hang detection, dirty flags, toggles.

use crate::app::{base_url, App};
use crate::ui::layout_manager::PanelId;
use tracing::{debug, info};

impl App {
    /// Seconds a watched session has been busy with no detectable activity.
    pub fn hang_silent_secs(&self, session_id: &str) -> Option<u64> {
        if !self.active_sessions.contains(session_id) {
            return None;
        }
        let _watcher = self.session_watchers.get(session_id)?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mcp_ms = self
            .last_mcp_activity_ms
            .load(std::sync::atomic::Ordering::Acquire);
        let pty_ms = self
            .session_ownership
            .get(session_id)
            .and_then(|&pidx| self.projects.get(pidx))
            .and_then(|p| p.ptys.get(session_id))
            .map(|pty| pty.last_output_at.load(std::sync::atomic::Ordering::Acquire))
            .unwrap_or(0);
        let msg_ms = self
            .last_message_event_at
            .get(session_id)
            .map(|inst| now_ms.saturating_sub(inst.elapsed().as_millis() as u64))
            .unwrap_or(0);
        let latest_ms = mcp_ms.max(pty_ms).max(msg_ms);
        if latest_ms == 0 {
            return Some(300);
        }
        Some(now_ms.saturating_sub(latest_ms) / 1000)
    }

    /// Check and consume the dirty flag on every rendered PTY.
    pub fn drain_pty_dirty_flags(&self) -> bool {
        let project = match self.projects.get(self.active_project) {
            Some(p) => p,
            None => return false,
        };
        let mut any_dirty = false;
        if let Some(pty) = project.active_pty() {
            any_dirty |= pty.take_dirty();
        }
        if let Some(resources) = project.active_resources() {
            for shell in &resources.shell_ptys {
                any_dirty |= shell.take_dirty();
            }
            if let Some(ref nvim) = resources.neovim_pty {
                any_dirty |= nvim.take_dirty();
            }
        }
        if let Some(ref gitui) = project.gitui_pty {
            any_dirty |= gitui.take_dirty();
        }
        any_dirty
    }

    #[allow(dead_code)]
    pub fn toggle_sidebar(&mut self) {
        self.layout.toggle_visible(PanelId::Sidebar);
        self.resize_all_ptys();
    }

    #[allow(dead_code)]
    pub fn toggle_focus(&mut self) {
        let panels = self.layout.visible_panels();
        if panels.len() < 2 {
            return;
        }
        let idx = panels
            .iter()
            .position(|&p| p == self.layout.focused)
            .unwrap_or(0);
        let next = (idx + 1) % panels.len();
        self.layout.focused = panels[next];
    }

    pub fn toggle_cheatsheet(&mut self) {
        self.show_cheatsheet = !self.show_cheatsheet;
    }

    /// Close the todo panel. If dirty, send a system message to the AI session.
    pub fn close_todo_panel(&mut self) {
        if let Some(panel) = self.todo_panel.take() {
            if panel.dirty {
                let session_id = panel.session_id.clone();
                info!(session_id, "Todo panel closed with changes");
                let proj_dir = self
                    .projects
                    .iter()
                    .find(|p| p.active_session.as_deref() == Some(&session_id))
                    .map(|p| p.path.to_string_lossy().to_string());
                if let Some(proj_dir) = proj_dir {
                    let base = base_url().to_string();
                    tokio::spawn(async move {
                        let client = crate::api::ApiClient::new();
                        let msg = "[SYSTEM REMINDER - TODO CONTINUATION] The todo list has been \
                                   updated. Re-read your todos and adjust your work plan accordingly. \
                                   Mark completed items done and continue with the next pending task.";
                        if let Err(e) = client
                            .send_system_message_async(&base, &proj_dir, &session_id, msg)
                            .await
                        {
                            tracing::error!("Failed to send todo continuation prompt: {e}");
                        }
                    });
                } else {
                    tracing::warn!(session_id, "Could not find project for session");
                }
            } else {
                debug!("Todo panel closed without changes");
            }
        }
    }
}
