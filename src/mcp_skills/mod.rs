//! Skills: reusable instruction files under `~/.config/opman/skills/<name>/SKILL.md`,
//! exposed to every runner through `opman mcp-skills`.
//!
//! Skills used to be reachable only over the web server's `POST /api/mcp`, which meant
//! no runner could ever see them — that endpoint is not in any runner's MCP list, and it
//! is not a conformant MCP transport either. A stdio server is the one channel every
//! runner speaks, so it is the only portable way to deliver skills to all of them.

pub mod bridge;
pub mod format;
pub mod name;
pub mod store;
pub mod tools;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

pub use name::{SkillName, SkillNameError};

/// One skill, keyed by its directory name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Directory name — the authoritative identity, and the only thing paths are built
    /// from.
    pub name: SkillName,
    /// Frontmatter `name`, for display. Defaults to the directory name.
    pub title: String,
    pub description: String,
    pub content: String,
    /// `mcp.json` servers this skill needs, so opman can say which login is missing.
    #[serde(default)]
    pub requires: Vec<String>,
}

/// Ordered, so the web UI's list stops reshuffling on every fetch — the old `HashMap`
/// iteration order was arbitrary per process.
pub type SkillsRegistry = Arc<RwLock<BTreeMap<SkillName, Skill>>>;

/// `$OPMAN_SKILLS_DIR`, else `~/.config/opman/skills`.
pub fn get_skills_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("OPMAN_SKILLS_DIR") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opman")
        .join("skills")
}

pub async fn load_skills() -> Result<BTreeMap<SkillName, Skill>> {
    Ok(load_skills_from(&get_skills_dir()))
}

/// Walk a skills directory. Never fails: one malformed `SKILL.md` must not take every
/// other skill down with it, for every runner.
pub fn load_skills_from(root: &Path) -> BTreeMap<SkillName, Skill> {
    let mut skills = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return skills;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| SkillName::parse(n).ok())
        else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(path.join("SKILL.md")) else {
            continue;
        };
        match format::parse_skill_md(&raw, &name) {
            Ok(skill) => {
                skills.insert(name, skill);
            }
            Err(error) => tracing::warn!(skill = %name, "skipping malformed SKILL.md: {error}"),
        }
    }
    skills
}

/// Re-read the registry whenever something writes a skill.
///
/// Renamed from `spawn_mcp_skills_server`, which started no server. Its loop also spun a
/// core at 100% once the sender dropped: `recv` then returns `Err(Closed)` forever, and
/// the old body ignored the error and looped.
pub fn spawn_skills_reload_watcher(mut reload_rx: broadcast::Receiver<()>, registry: SkillsRegistry) {
    tokio::spawn(async move {
        loop {
            match reload_rx.recv().await {
                Ok(()) => {
                    *registry.write().await = load_skills_from(&get_skills_dir());
                    tracing::info!("reloaded skills registry");
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!("skills reload watcher lagged {n} messages");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
