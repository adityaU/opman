use std::path::PathBuf;

use anyhow::Result;

use crate::app::App;
use crate::app::{InputMode, Project, ServerStatus};
use crate::config::ProjectEntry;
use crate::ui::fuzzy_picker::FuzzyPickerState;

impl App {
    /// Switch the active project by index.
    pub fn switch_project(&mut self, index: usize) {
        if index < self.projects.len() {
            self.active_project = index;
            self.resize_all_ptys();
        }
    }

    pub fn add_project(&mut self, entry: ProjectEntry) {
        let project = Project {
            name: entry.name.clone(),
            path: std::fs::canonicalize(&entry.path).unwrap_or_else(|_| PathBuf::from(&entry.path)),
            ptys: std::collections::HashMap::new(),
            active_session: None,
            session_resources: std::collections::HashMap::new(),
            gitui_pty: None,
            sessions: Vec::new(),
            git_branch: String::new(),
        };
        self.projects.push(project);
        self.config.projects.push(entry);
    }

    pub fn start_add_project(&mut self) {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let existing: Vec<String> = self
            .projects
            .iter()
            .map(|p| p.path.to_string_lossy().to_string())
            .collect();
        self.fuzzy_picker = Some(FuzzyPickerState::new_with_existing(home, existing));
        self.input_mode = InputMode::FuzzyPicker;
    }

    /// Cancel the fuzzy picker and return to normal mode.
    pub fn cancel_fuzzy_picker(&mut self) {
        self.fuzzy_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Confirm the fuzzy picker selection and add the project.
    pub fn confirm_fuzzy_add_project(&mut self) -> Result<()> {
        let selected_path = self
            .fuzzy_picker
            .as_ref()
            .and_then(|fp| fp.selected_path())
            .map(|s| s.to_string());

        self.fuzzy_picker = None;
        self.input_mode = InputMode::Normal;

        let path_str = match selected_path {
            Some(p) => p,
            None => return Ok(()),
        };

        // Expand ~ to home directory
        let expanded = if path_str.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(&path_str[1..].trim_start_matches('/'))
                    .to_string_lossy()
                    .to_string()
            } else {
                path_str.clone()
            }
        } else {
            path_str.clone()
        };

        let path = PathBuf::from(&expanded);
        if !path.is_dir() {
            return Ok(());
        }

        // If project already exists, switch to it instead of adding
        for (i, project) in self.projects.iter().enumerate() {
            if project.path == path {
                self.switch_project(i);
                return Ok(());
            }
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());

        let entry = ProjectEntry {
            name,
            path: path_str,
            terminal_command: None,
        };
        self.add_project(entry);
        self.config.save()?;
        Ok(())
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.clear_completions();
    }

    pub fn confirm_add_project(&mut self) -> Result<()> {
        let raw = self.input_buffer.trim().to_string();
        let path_str = self.expand_tilde(&raw);
        let path = PathBuf::from(&path_str);

        if !path.is_dir() {
            self.cancel_input();
            return Ok(());
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());

        let entry = ProjectEntry {
            name,
            path: path_str,
            terminal_command: None,
        };
        self.add_project(entry);
        self.config.save()?;

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.clear_completions();
        Ok(())
    }

    pub fn remove_project(&mut self, index: usize) -> Result<()> {
        if index >= self.projects.len() {
            return Ok(());
        }

        let mut project = self.projects.remove(index);

        for (_, pty) in project.ptys.iter_mut() {
            let _ = pty.kill();
        }
        project.ptys.clear();
        drop(project);

        self.config.projects.remove(index);
        self.config.save()?;

        if self.projects.is_empty() {
            self.active_project = 0;
            self.sidebar_selection = 0;
            self.sidebar_cursor = 0;
        } else {
            if self.active_project >= self.projects.len() {
                self.active_project = self.projects.len().saturating_sub(1);
            }
            let max = self.projects.len().saturating_sub(1);
            self.sidebar_selection = self.sidebar_selection.min(max);
            self.sidebar_cursor = self.sidebar_cursor.min(max);
        }

        Ok(())
    }

    /// Derive the server status for a given project.
    /// With the shared server architecture, we always report Running.
    pub fn project_server_status(&self, _index: usize) -> ServerStatus {
        ServerStatus::Running
    }
}
