use crate::app::{App, SessionResources};
use crate::mcp::{SocketResponse, TabInfo};
use crate::pty::PtyInstance;

impl App {
    /// Handle an incoming MCP socket request for a given project/session.
    ///
    /// This dispatches terminal operations (read, run, list, new, close, rename,
    /// status) directly, and delegates neovim operations to
    /// [`handle_nvim_operation`](Self::handle_nvim_operation).
    pub(crate) fn handle_mcp_request(
        &mut self,
        project_idx: usize,
        session_id: &str,
        request: &crate::mcp::SocketRequest,
    ) -> SocketResponse {
        // Collect spawn parameters before borrowing project mutably.
        let shell_size = self
            .layout
            .panel_rect(crate::ui::layout_manager::PanelId::IntegratedTerminal)
            .map(|r| (r.height.saturating_sub(1).max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let nvim_size = self
            .layout
            .panel_rect(crate::ui::layout_manager::PanelId::NeovimPane)
            .map(|r| (r.height.max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = crate::theme_gen::theme_dir();
        let terminal_command = self
            .config
            .projects
            .get(project_idx)
            .and_then(|e| e.terminal_command.as_deref())
            .or(self.config.settings.default_terminal_command.as_deref())
            .map(|s| s.to_string());

        let project = match self.projects.get_mut(project_idx) {
            Some(p) => p,
            None => return SocketResponse::err("Project not found".into()),
        };
        let project_path = project.path.clone();
        let resources = project
            .session_resources
            .entry(session_id.to_string())
            .or_insert_with(SessionResources::new);

        // Determine if this is a terminal op that needs at least one shell tab.
        let needs_shell = matches!(
            request.op.as_str(),
            "read" | "run" | "close" | "rename" | "status"
        );
        // Determine if this is a neovim op.
        let needs_neovim = request.op.starts_with("nvim_");

        // Lazily spawn a shell PTY if needed and none exist.
        if needs_shell && resources.shell_ptys.is_empty() {
            match PtyInstance::spawn_shell(
                shell_size.0,
                shell_size.1,
                &project_path,
                &theme_envs,
                Some(&td),
                terminal_command.as_deref(),
                None,
            ) {
                Ok(shell) => {
                    resources.shell_ptys.push(shell);
                    resources.active_shell_tab = 0;
                }
                Err(e) => {
                    return SocketResponse::err(format!("Failed to auto-start terminal: {}", e));
                }
            }
        }

        // Lazily spawn neovim if needed and not running.
        if needs_neovim && resources.neovim_pty.is_none() {
            match PtyInstance::spawn_neovim(
                nvim_size.0,
                nvim_size.1,
                &project_path,
                &theme_envs,
                Some(&td),
                Some(session_id),
            ) {
                Ok(nvim) => {
                    // Register nvim socket in shared registry for off-main-loop handling.
                    if let Some(ref addr) = nvim.nvim_listen_addr {
                        let reg = self.nvim_registry.clone();
                        let key = (project_idx, session_id.to_string());
                        let addr = addr.clone();
                        tokio::spawn(async move {
                            reg.write().await.insert(key, addr);
                        });
                    }
                    resources.neovim_pty = Some(nvim);
                }
                Err(e) => {
                    return SocketResponse::err(format!("Failed to auto-start neovim: {}", e));
                }
            }
        }

        match request.op.as_str() {
            "read" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                match resources.shell_ptys.get(tab_idx) {
                    Some(pty) => {
                        if let Ok(mut parser) = pty.parser.lock() {
                            let text = super::read_full_terminal_buffer(&mut parser, request.last_n);
                            SocketResponse::ok_text(text)
                        } else {
                            SocketResponse::err("Failed to lock terminal parser".into())
                        }
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            "run" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                let command = match &request.command {
                    Some(c) => c,
                    None => return SocketResponse::err("Missing 'command' for run op".into()),
                };
                match resources.shell_ptys.get_mut(tab_idx) {
                    Some(pty) => {
                        let is_ctrl_c = command == "\x03";
                        if !is_ctrl_c {
                            if let Ok(state) = pty.command_state.lock() {
                                if *state == crate::pty::CommandState::Running {
                                    return SocketResponse::err(
                                        "Tab is busy (command running). Send Ctrl-C (\\x03) to interrupt, try another tab, or use terminal_ephemeral_run.".into()
                                    );
                                }
                            }
                        }
                        let bytes = if is_ctrl_c {
                            command.as_bytes().to_vec()
                        } else {
                            format!("{}\n", command).into_bytes()
                        };
                        match pty.write(&bytes) {
                            Ok(_) => {
                                SocketResponse::ok_text(format!("Command sent to tab {}", tab_idx))
                            }
                            Err(e) => {
                                SocketResponse::err(format!("Failed to write to terminal: {}", e))
                            }
                        }
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            "list" => {
                let tabs: Vec<TabInfo> = resources
                    .shell_ptys
                    .iter()
                    .enumerate()
                    .map(|(i, pty)| TabInfo {
                        index: i,
                        active: i == resources.active_shell_tab,
                        name: if pty.name.is_empty() {
                            format!("Tab {}", i + 1)
                        } else {
                            pty.name.clone()
                        },
                    })
                    .collect();
                SocketResponse::ok_tabs(tabs)
            }
            "new" => {
                match PtyInstance::spawn_shell(
                    shell_size.0,
                    shell_size.1,
                    &project_path,
                    &theme_envs,
                    Some(&td),
                    terminal_command.as_deref(),
                    request.name.clone(),
                ) {
                    Ok(shell) => {
                        resources.shell_ptys.push(shell);
                        let new_idx = resources.shell_ptys.len() - 1;
                        resources.active_shell_tab = new_idx;
                        SocketResponse::ok_tab_created(new_idx)
                    }
                    Err(e) => SocketResponse::err(format!("Failed to spawn shell: {}", e)),
                }
            }
            "close" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                if tab_idx >= resources.shell_ptys.len() {
                    return SocketResponse::err(format!("Tab {} not found", tab_idx));
                }
                if resources.shell_ptys.len() <= 1 {
                    return SocketResponse::err("Cannot close the last tab".into());
                }
                let mut pty = resources.shell_ptys.remove(tab_idx);
                let _ = pty.kill();
                if resources.active_shell_tab >= resources.shell_ptys.len() {
                    resources.active_shell_tab = resources.shell_ptys.len().saturating_sub(1);
                }
                SocketResponse::ok_empty()
            }
            "rename" => {
                let tab_idx = match request.tab {
                    Some(idx) => idx,
                    None => return SocketResponse::err("Missing 'tab' for rename op".into()),
                };
                let name = match &request.name {
                    Some(n) => n,
                    None => return SocketResponse::err("Missing 'name' for rename op".into()),
                };
                match resources.shell_ptys.get_mut(tab_idx) {
                    Some(pty) => {
                        pty.name = name.clone();
                        SocketResponse::ok_empty()
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            "status" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                match resources.shell_ptys.get(tab_idx) {
                    Some(pty) => {
                        let state = if let Ok(cs) = pty.command_state.lock() {
                            match *cs {
                                crate::pty::CommandState::Idle => "idle",
                                crate::pty::CommandState::Running => "running",
                                crate::pty::CommandState::Success => "success",
                                crate::pty::CommandState::Failure => "failure",
                            }
                        } else {
                            "unknown"
                        };
                        SocketResponse::ok_status(state.to_string())
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            // ── Neovim operations ─────────────────────────────────────
            "nvim_open" | "nvim_read" | "nvim_command" | "nvim_buffers" | "nvim_info"
            | "nvim_diagnostics" | "nvim_definition" | "nvim_references" | "nvim_hover"
            | "nvim_symbols" | "nvim_code_actions" | "nvim_eval" | "nvim_grep" | "nvim_diff"
            | "nvim_write" | "nvim_edit_and_save" | "nvim_undo" | "nvim_rename" | "nvim_format"
            | "nvim_signature" => {
                // Resolve neovim socket address.
                let nvim_socket = match &resources.neovim_pty {
                    Some(pty) => match &pty.nvim_listen_addr {
                        Some(addr) => addr.clone(),
                        None => {
                            return SocketResponse::err(
                                "Neovim PTY has no listen address".into(),
                            )
                        }
                    },
                    None => {
                        return SocketResponse::err(
                            "Neovim is not running for this project. Focus the Neovim pane to start it.".into(),
                        )
                    }
                };

                // Resolve file_path → buffer handle (0 = current buffer).
                let buf: i64 = if let Some(ref path) = request.file_path {
                    if request.op != "nvim_open" {
                        match crate::nvim_rpc::nvim_find_or_load_buffer(&nvim_socket, path) {
                            Ok(id) => id,
                            Err(e) => {
                                return SocketResponse::err(format!(
                                    "Failed to resolve buffer for '{}': {}",
                                    path, e
                                ))
                            }
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };

                Self::handle_nvim_operation(&nvim_socket, buf, request)
            }
            other => SocketResponse::err(format!("Unknown operation: {}", other)),
        }
    }
}
