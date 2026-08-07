//! Which server to run, and where to run it.
//!
//! Two questions, both answered from the file path alone: what language is
//! this, and which directory is its project root. The root matters more than it
//! looks — rust-analyzer given the wrong directory indexes the wrong crate
//! graph and reports nonsense, and a server per subdirectory would run several
//! copies of a program that costs gigabytes.
//!
//! The table lists servers we know how to talk to; whether any given one is
//! installed is discovered at spawn time, so an absent binary degrades to "no
//! LSP for this file" rather than an error.

use std::path::{Path, PathBuf};

/// A language server's `languageId` string, as the protocol spells it.
pub type LanguageId = &'static str;

/// How to start one server, and what its project looks like.
#[derive(Clone, Copy)]
pub struct ServerSpec {
    pub language: LanguageId,
    pub command: &'static str,
    pub args: &'static [&'static str],
    /// Files whose presence marks a project root, nearest-first in priority.
    pub roots: &'static [&'static str],
}

/// Extension → language. Kept separate from the server table because several
/// extensions share one server.
const EXTENSIONS: &[(&str, LanguageId)] = &[
    ("rs", "rust"),
    ("ts", "typescript"),
    ("mts", "typescript"),
    ("cts", "typescript"),
    ("tsx", "typescriptreact"),
    ("js", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("jsx", "javascriptreact"),
    ("py", "python"),
    ("pyi", "python"),
    ("go", "go"),
    ("lua", "lua"),
    ("c", "c"),
    ("h", "c"),
    ("cc", "cpp"),
    ("cpp", "cpp"),
    ("cxx", "cpp"),
    ("hpp", "cpp"),
    ("sh", "shellscript"),
    ("bash", "shellscript"),
    ("json", "json"),
    ("jsonc", "json"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("css", "css"),
    ("scss", "scss"),
    ("html", "html"),
];

const SERVERS: &[ServerSpec] = &[
    ServerSpec {
        language: "rust",
        command: "rust-analyzer",
        args: &[],
        roots: &["Cargo.toml", "rust-project.json"],
    },
    ServerSpec {
        language: "typescript",
        command: "typescript-language-server",
        args: &["--stdio"],
        roots: &["tsconfig.json", "jsconfig.json", "package.json"],
    },
    ServerSpec {
        language: "python",
        command: "pyright-langserver",
        args: &["--stdio"],
        roots: &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"],
    },
    ServerSpec {
        language: "go",
        command: "gopls",
        args: &[],
        roots: &["go.mod", "go.work"],
    },
    ServerSpec {
        language: "lua",
        command: "lua-language-server",
        args: &[],
        roots: &[".luarc.json", "stylua.toml"],
    },
    ServerSpec {
        language: "c",
        command: "clangd",
        args: &["--background-index"],
        roots: &["compile_commands.json", "compile_flags.txt", "CMakeLists.txt"],
    },
    ServerSpec {
        language: "shellscript",
        command: "bash-language-server",
        args: &["start"],
        roots: &[],
    },
    ServerSpec {
        language: "json",
        command: "vscode-json-language-server",
        args: &["--stdio"],
        roots: &["package.json"],
    },
    ServerSpec {
        language: "yaml",
        command: "yaml-language-server",
        args: &["--stdio"],
        roots: &[],
    },
    ServerSpec {
        language: "css",
        command: "vscode-css-language-server",
        args: &["--stdio"],
        roots: &["package.json"],
    },
];

/// Languages that share another language's server process.
fn canonical(language: LanguageId) -> LanguageId {
    match language {
        "typescriptreact" | "javascript" | "javascriptreact" => "typescript",
        "cpp" => "c",
        "scss" => "css",
        other => other,
    }
}

/// The `languageId` for a path, or `None` when we have no mapping — which is
/// how an unsupported file type reaches the caller as "no LSP here".
pub fn language_for(path: &Path) -> Option<LanguageId> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    EXTENSIONS
        .iter()
        .find(|(candidate, _)| *candidate == ext)
        .map(|(_, language)| *language)
}

/// The server that serves a language, if we know one.
pub fn spec_for(language: LanguageId) -> Option<ServerSpec> {
    let target = canonical(language);
    SERVERS.iter().copied().find(|s| s.language == target)
}

/// Directories searched for a server binary beyond `PATH`. Mason installs are
/// how most of these land on a developer machine, and they are not on `PATH`.
fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/nvim/mason/bin"));
        dirs.push(home.join(".cargo/bin"));
        dirs.push(home.join(".local/bin"));
    }
    dirs
}

/// Resolve a server command to an executable path, or `None` if not installed.
pub fn resolve_binary(command: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(command);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    extra_bin_dirs()
        .into_iter()
        .map(|dir| dir.join(command))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Nearest ancestor of `file` holding one of `markers`, never climbing above
/// `project_dir`. Falls back to the nearest `.git`, then to `project_dir`
/// itself — so a file in a repo with no build manifest still gets one stable
/// root rather than a server per directory.
pub fn project_root(file: &Path, project_dir: &Path, markers: &[&str]) -> PathBuf {
    let start = file.parent().unwrap_or(project_dir);

    if !markers.is_empty() {
        if let Some(found) = climb(start, project_dir, |dir| {
            markers.iter().any(|m| dir.join(m).exists())
        }) {
            return found;
        }
    }
    climb(start, project_dir, |dir| dir.join(".git").exists())
        .unwrap_or_else(|| project_dir.to_path_buf())
}

/// Walk from `start` up to and including `ceiling`, returning the first hit.
fn climb(start: &Path, ceiling: &Path, hit: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if hit(dir) {
            return Some(dir.to_path_buf());
        }
        if dir == ceiling {
            break;
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod detect_tests;
