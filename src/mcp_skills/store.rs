//! The skills view held by an `opman mcp-skills` child process.
//!
//! The child reads the skills directory directly rather than asking opman over the
//! loopback API. The directory *is* the source of truth — the web server's registry is
//! only a cache of it — and that descriptor exists only when opman runs with `--web`,
//! while opman's default mode is TUI-only. Coupling to it would mean skills silently do
//! not reach runners in the default mode, which is the exact bug this fixes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::name::SkillName;
use super::{load_skills_from, Skill};

/// How stale a listing may be before a re-scan. A `read_dir` plus a few small reads is
/// sub-millisecond, so this buys live edits for nothing.
const TTL: Duration = Duration::from_millis(250);

pub struct SkillStore {
    root: PathBuf,
    ttl: Duration,
    loaded_at: Option<Instant>,
    skills: BTreeMap<SkillName, Skill>,
    /// Tool name to skill, so a `tools/call` is a lookup rather than a re-slug of every
    /// skill on every call.
    tools: HashMap<String, SkillName>,
}

impl SkillStore {
    pub fn open(root: PathBuf) -> Self {
        Self {
            root,
            ttl: TTL,
            loaded_at: None,
            skills: BTreeMap::new(),
            tools: HashMap::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::open(super::get_skills_dir())
    }

    /// A store over an exact skill set, with the TTL already satisfied so nothing
    /// re-scans a directory. Keeps tool tests off the filesystem entirely.
    #[cfg(test)]
    pub(crate) fn seeded(skills: Vec<Skill>) -> Self {
        let skills: BTreeMap<SkillName, Skill> = skills
            .into_iter()
            .map(|skill| (skill.name.clone(), skill))
            .collect();
        let tools = super::tools::index(&skills);
        Self {
            root: PathBuf::new(),
            ttl: Duration::from_secs(3600),
            loaded_at: Some(Instant::now()),
            skills,
            tools,
        }
    }

    /// Re-scan if the listing is stale. Returns whether the *tool set* changed, so the
    /// bridge can tell the runner its tool list moved.
    pub fn refresh(&mut self) -> bool {
        if self.loaded_at.is_some_and(|at| at.elapsed() < self.ttl) {
            return false;
        }
        self.loaded_at = Some(Instant::now());
        let skills = load_skills_from(&self.root);
        let tools = super::tools::index(&skills);
        let changed = tools.keys().ne(self.tools.keys());
        self.skills = skills;
        self.tools = tools;
        changed
    }

    pub fn skills(&self) -> &BTreeMap<SkillName, Skill> {
        &self.skills
    }

    pub fn by_tool(&self, tool: &str) -> Option<&Skill> {
        self.tools.get(tool).and_then(|name| self.skills.get(name))
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        SkillName::parse(name)
            .ok()
            .and_then(|name| self.skills.get(&name))
    }

    /// Every `mcp.json` server any skill declares a dependency on.
    pub fn required_servers(&self) -> BTreeSet<&str> {
        self.skills
            .values()
            .flat_map(|skill| skill.requires.iter().map(String::as_str))
            .collect()
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod store_tests;
