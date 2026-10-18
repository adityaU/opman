//! Which helper programs are on the PATH, and what each one is for.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::report::Check;

struct Tool {
    name: &'static str,
    purpose: &'static str,
    /// Missing is a failure even if no ACP agent needs it.
    always_needed: bool,
}

const fn tool(name: &'static str, purpose: &'static str) -> Tool {
    Tool {
        name,
        purpose,
        always_needed: false,
    }
}

const TOOLS: &[Tool] = &[
    Tool {
        name: "git",
        purpose: "the git panel, worktrees and project detection",
        always_needed: true,
    },
    tool("node", "runs npm-based ACP agents (they start with npx)"),
    tool(
        "npx",
        "starts npm-based ACP agents such as the Claude and Codex adapters",
    ),
    tool("uv", "manages Python tools for uvx-based ACP agents"),
    tool("uvx", "starts Python-based ACP agents"),
    tool(
        "nvim",
        "optional: Neovim MCP bridge for agents that edit through Neovim",
    ),
    tool("cloudflared", "optional: public tunnels for remote access"),
    tool("docker", "optional: sandboxed or containerised agents"),
    tool(
        "claude",
        "the Claude Code CLI (Claude runner and its login)",
    ),
    tool("codex", "the Codex CLI (Codex runner and its login)"),
];

/// The program a launch command runs: `npx` for `/usr/bin/npx`.
pub(super) fn command_name(command: &str) -> &str {
    Path::new(command)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(command)
}

/// The first executable named `name` in `path`. A name with a slash is taken as it is.
pub(super) fn find_on_path(name: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    if name.contains('/') {
        let direct = PathBuf::from(name);
        return is_executable(&direct).then_some(direct);
    }
    std::env::split_paths(path?)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// One check per tool. `commands` are the launch commands of the enabled ACP agents: a tool
/// one of them runs is required, any other missing tool is only a warning.
pub(super) fn checks(commands: &[&str], path: Option<&OsStr>) -> Vec<Check> {
    let mut needed: HashSet<&str> = commands.iter().map(|c| command_name(c)).collect();
    // `npx` is a script that runs `node`, and `uvx` ships with `uv`.
    for (runner, base) in [("npx", "node"), ("uvx", "uv")] {
        if needed.contains(runner) {
            needed.insert(base);
        }
    }
    TOOLS
        .iter()
        .map(|tool| check(tool, needed.contains(tool.name), path))
        .collect()
}

fn check(tool: &Tool, needed_by_agent: bool, path: Option<&OsStr>) -> Check {
    if let Some(found) = find_on_path(tool.name, path) {
        return Check::pass(format!("{} found at {}", tool.name, found.display()))
            .detail(tool.purpose);
    }
    let title = format!("{} not found on PATH", tool.name);
    if tool.always_needed || needed_by_agent {
        let why = if needed_by_agent {
            "an enabled ACP agent needs it. "
        } else {
            ""
        };
        return Check::fail(title).detail(format!("{why}Used for: {}", tool.purpose));
    }
    Check::warn(title).detail(format!("Used for: {}", tool.purpose))
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tools_tests;
