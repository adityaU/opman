//! Where relative paths start, and how paths are shown back to the model.
//!
//! Rule: a relative path means "inside the project". An absolute path, or one starting
//! with `~`, is used as given, even outside the project, the same as the native tools.
//! This is a convenience, not a sandbox.

use crate::mcp_tools::output::ToolError;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Self {
        let root = std::path::absolute(&root).unwrap_or(root);
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, raw: &str) -> Result<PathBuf, ToolError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(ToolError::new("path is empty"));
        }
        if raw == "~" || raw.starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| ToolError::new("cannot find the home directory for '~'"))?;
            return Ok(home.join(raw.trim_start_matches('~').trim_start_matches('/')));
        }
        Ok(self.root.join(raw))
    }

    /// The path relative to the project when it is inside it, else as is.
    pub fn show(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}
