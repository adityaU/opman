use crate::app::App;
use crate::app::SessionResources;
use crate::pty::PtyInstance;
use crate::theme_gen;
use crate::ui::layout_manager::PanelId;

impl App {
    pub fn resize_all_ptys(&mut self) {
        let area = self.layout.last_area;
        self.layout.compute_rects(area);

        let term_rect = self.layout.panel_rect(PanelId::TerminalPane);
        let shell_rect = self.layout.panel_rect(PanelId::IntegratedTerminal);
        let nvim_rect = self.layout.panel_rect(PanelId::NeovimPane);
        let git_rect = self.layout.panel_rect(PanelId::GitPanel);

        if let Some(project) = self.projects.get_mut(self.active_project) {
            if let (Some(pty), Some(rect)) = (project.active_pty_mut(), term_rect) {
                if rect.width > 0 && rect.height > 0 {
                    let _ = pty.resize(rect.height, rect.width);
                }
            }
            if let Some(resources) = project.active_resources_mut() {
                if let Some(rect) = shell_rect {
                    if rect.width > 0 && rect.height > 0 {
                        let content_height = rect.height.saturating_sub(1).max(1);
                        for shell_pty in &mut resources.shell_ptys {
                            let _ = shell_pty.resize(content_height, rect.width);
                        }
                    }
                }
                if let (Some(ref mut nvim_pty), Some(rect)) = (&mut resources.neovim_pty, nvim_rect)
                {
                    if rect.width > 0 && rect.height > 0 {
                        let _ = nvim_pty.resize(rect.height, rect.width);
                    }
                }
            }
            if let (Some(ref mut gitui_pty), Some(rect)) = (&mut project.gitui_pty, git_rect) {
                if rect.width > 0 && rect.height > 0 {
                    let _ = gitui_pty.resize(rect.height, rect.width);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn terminal_inner_size(&self, total_rows: u16, total_cols: u16) -> (u16, u16) {
        self.layout.panel_rect(PanelId::TerminalPane)
            .map(|r| (r.height, r.width))
            .unwrap_or((total_rows.saturating_sub(1), total_cols))
    }

    #[allow(dead_code)]
    pub fn shell_terminal_inner_size(&self, _total_rows: u16, _total_cols: u16) -> (u16, u16) {
        self.layout.panel_rect(PanelId::IntegratedTerminal)
            .map(|r| (r.height, r.width))
            .unwrap_or((0, 0))
    }

    #[allow(dead_code)]
    pub fn terminal_pane_offset(&self) -> u16 {
        self.layout.panel_rect(PanelId::TerminalPane).map(|r| r.x).unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn neovim_terminal_inner_size(&self) -> (u16, u16) {
        self.layout.panel_rect(PanelId::NeovimPane)
            .map(|r| (r.height, r.width))
            .unwrap_or((0, 0))
    }

    pub fn ensure_shell_pty(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        let sid = match self.projects[index].active_session.clone() {
            Some(s) => s,
            None => return,
        };
        {
            let resources = self.projects[index]
                .session_resources
                .entry(sid.clone())
                .or_insert_with(SessionResources::new);
            if !resources.shell_ptys.is_empty() {
                return;
            }
        }
        let shell_rows = self
            .layout
            .panel_rect(PanelId::IntegratedTerminal)
            .map(|r| (r.height.saturating_sub(1).max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = theme_gen::theme_dir();
        let command = self
            .config
            .projects
            .get(index)
            .and_then(|e| e.terminal_command.as_deref())
            .or(self.config.settings.default_terminal_command.as_deref());

        match PtyInstance::spawn_shell(
            shell_rows.0,
            shell_rows.1,
            &self.projects[index].path,
            &theme_envs,
            Some(&td),
            command,
            None,
        ) {
            Ok(shell) => {
                let resources = self.projects[index]
                    .session_resources
                    .entry(sid)
                    .or_insert_with(SessionResources::new);
                resources.shell_ptys.push(shell);
                resources.active_shell_tab = 0;
            }
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn shell PTY: {}", e
            ),
        }
    }

    pub fn add_shell_tab(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        let sid = match self.projects[index].active_session.clone() {
            Some(s) => s,
            None => return,
        };
        let shell_rows = self
            .layout
            .panel_rect(PanelId::IntegratedTerminal)
            .map(|r| (r.height.saturating_sub(1).max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = theme_gen::theme_dir();
        let command = self
            .config
            .projects
            .get(index)
            .and_then(|e| e.terminal_command.as_deref())
            .or(self.config.settings.default_terminal_command.as_deref());

        match PtyInstance::spawn_shell(
            shell_rows.0,
            shell_rows.1,
            &self.projects[index].path,
            &theme_envs,
            Some(&td),
            command,
            None,
        ) {
            Ok(shell) => {
                let resources = self.projects[index]
                    .session_resources
                    .entry(sid)
                    .or_insert_with(SessionResources::new);
                resources.shell_ptys.push(shell);
                resources.active_shell_tab = resources.shell_ptys.len() - 1;
            }
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn shell tab PTY: {}", e
            ),
        }
    }

    pub fn ensure_neovim_pty(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        let sid = match self.projects[index].active_session.clone() {
            Some(s) => s,
            None => return,
        };
        {
            let resources = self.projects[index]
                .session_resources
                .entry(sid.clone())
                .or_insert_with(SessionResources::new);
            if resources.neovim_pty.is_some() {
                return;
            }
        }
        let nvim_size = self
            .layout
            .panel_rect(PanelId::NeovimPane)
            .map(|r| (r.height.max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = theme_gen::theme_dir();
        match PtyInstance::spawn_neovim(
            nvim_size.0,
            nvim_size.1,
            &self.projects[index].path,
            &theme_envs,
            Some(&td),
            Some(&sid),
        ) {
            Ok(nvim) => {
                if let Some(ref addr) = nvim.nvim_listen_addr {
                    let reg = self.nvim_registry.clone();
                    let key = (index, sid.clone());
                    let addr = addr.clone();
                    tokio::spawn(async move {
                        reg.write().await.insert(key, addr);
                    });
                }
                let resources = self.projects[index]
                    .session_resources
                    .entry(sid)
                    .or_insert_with(SessionResources::new);
                resources.neovim_pty = Some(nvim);
            }
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn neovim PTY: {}", e
            ),
        }
    }

    pub fn ensure_gitui_pty(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        if self.projects[index].gitui_pty.is_some() {
            return;
        }
        let git_size = self
            .layout
            .panel_rect(PanelId::GitPanel)
            .map(|r| (r.height.max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_path = theme_gen::theme_dir().join("gitui/opencode.ron");
        let theme_ref = if theme_path.exists() {
            Some(theme_path.as_path())
        } else {
            None
        };
        match PtyInstance::spawn_gitui(
            git_size.0,
            git_size.1,
            &self.projects[index].path,
            theme_ref,
        ) {
            Ok(pty) => self.projects[index].gitui_pty = Some(pty),
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn gitui PTY: {}", e
            ),
        }
    }

    /// Update running PTY programs when the theme changes.
    pub fn update_ptys_for_theme(&mut self) {
        let is_dark = {
            if let ratatui::style::Color::Rgb(r, g, b) = self.theme.background {
                let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
                lum < 128.0
            } else {
                true
            }
        };
        let bg = if is_dark { "dark" } else { "light" };
        let colorscheme_path = theme_gen::theme_dir().join("nvim/colors/opencode.lua");
        let nvim_cmd = format!(
            "\x1b:set background={} | luafile {}\r",
            bg,
            colorscheme_path.display()
        );
        let theme_dir = theme_gen::theme_dir();
        let zsh_theme = theme_dir.join("opencode.zsh");
        let shell_cmd = format!(" source '{}'; clear\n", zsh_theme.display());

        for project in self.projects.iter_mut() {
            for resources in project.session_resources.values_mut() {
                if let Some(ref mut nvim) = resources.neovim_pty {
                    let _ = nvim.write(nvim_cmd.as_bytes());
                }
                for shell in &mut resources.shell_ptys {
                    let _ = shell.write(shell_cmd.as_bytes());
                }
            }
            if project.gitui_pty.is_some() {
                project.gitui_pty = None;
            }
        }
    }
}
