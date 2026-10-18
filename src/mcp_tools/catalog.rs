//! The tools this server offers, and the one place that turns a call into a result.

use super::output::{ToolError, ToolResult};
use super::workspace::Workspace;
use super::{bash, edit_file, glob, grep, list_dir, read_file, web_fetch, write_file};
use super::{catalog_files as files, catalog_search as search};
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Bash,
    ReadFile,
    WriteFile,
    EditFile,
    ListDir,
    Glob,
    Grep,
    WebFetch,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Bash,
        Tool::ReadFile,
        Tool::WriteFile,
        Tool::EditFile,
        Tool::ListDir,
        Tool::Glob,
        Tool::Grep,
        Tool::WebFetch,
    ];

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Bash => "bash",
            Tool::ReadFile => "read_file",
            Tool::WriteFile => "write_file",
            Tool::EditFile => "edit_file",
            Tool::ListDir => "list_dir",
            Tool::Glob => "glob",
            Tool::Grep => "grep",
            Tool::WebFetch => "web_fetch",
        }
    }

    fn definition(self) -> Value {
        match self {
            Tool::Bash => search::bash(),
            Tool::ReadFile => files::read_file(),
            Tool::WriteFile => files::write_file(),
            Tool::EditFile => files::edit_file(),
            Tool::ListDir => files::list_dir(),
            Tool::Glob => search::glob(),
            Tool::Grep => search::grep(),
            Tool::WebFetch => search::web_fetch(),
        }
    }

    pub async fn call(self, workspace: &Workspace, arguments: Value) -> ToolResult {
        match self {
            Tool::Bash => bash::run(workspace, parse(arguments)?).await,
            Tool::ReadFile => read_file::run(workspace, parse(arguments)?).await,
            Tool::WriteFile => write_file::run(workspace, parse(arguments)?).await,
            Tool::EditFile => edit_file::run(workspace, parse(arguments)?).await,
            Tool::ListDir => list_dir::run(workspace, parse(arguments)?).await,
            Tool::Glob => glob::run(workspace, parse(arguments)?).await,
            Tool::Grep => grep::run(workspace, parse(arguments)?).await,
            Tool::WebFetch => web_fetch::run(parse(arguments)?).await,
        }
    }
}

/// The `tools/list` result body.
pub fn definitions() -> Value {
    Value::Array(Tool::ALL.into_iter().map(Tool::definition).collect())
}

fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, ToolError> {
    let arguments = match arguments {
        Value::Null => Value::Object(Default::default()),
        other => other,
    };
    serde_json::from_value(arguments).map_err(|e| ToolError::new(format!("invalid arguments: {e}")))
}
