//! Panel visibility and focus state.

use super::WebStateHandle;
use crate::web::types::WebEvent;

impl WebStateHandle {
    /// Toggle a panel's visibility.
    pub async fn toggle_panel(&self, panel: &str) -> bool {
        let mut inner = self.inner.write().await;
        let ok = match panel {
            "Sidebar" | "sidebar" => {
                inner.panels.sidebar = !inner.panels.sidebar;
                true
            }
            "TerminalPane" | "terminal_pane" => {
                inner.panels.terminal_pane = !inner.panels.terminal_pane;
                true
            }
            "NeovimPane" | "neovim_pane" => {
                inner.panels.neovim_pane = !inner.panels.neovim_pane;
                true
            }
            "IntegratedTerminal" | "integrated_terminal" => {
                inner.panels.integrated_terminal = !inner.panels.integrated_terminal;
                true
            }
            "GitPanel" | "git_panel" => {
                inner.panels.git_panel = !inner.panels.git_panel;
                true
            }
            _ => false,
        };
        if ok {
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
        ok
    }

    /// Set the focused panel.
    pub async fn focus_panel(&self, panel: &str) -> bool {
        let valid = matches!(
            panel,
            "Sidebar"
                | "sidebar"
                | "TerminalPane"
                | "terminal_pane"
                | "NeovimPane"
                | "neovim_pane"
                | "IntegratedTerminal"
                | "integrated_terminal"
                | "GitPanel"
                | "git_panel"
        );
        if valid {
            let mut inner = self.inner.write().await;
            // Normalize to PascalCase
            inner.focused = match panel {
                "sidebar" => "Sidebar",
                "terminal_pane" => "TerminalPane",
                "neovim_pane" => "NeovimPane",
                "integrated_terminal" => "IntegratedTerminal",
                "git_panel" => "GitPanel",
                other => other,
            }
            .to_string();
            let _ = self.event_tx.send(WebEvent::StateChanged);
            true
        } else {
            false
        }
    }
}
