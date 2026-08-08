//! Descriptions for the slash commands claude reports.
//!
//! Claude's `system/init` event names its slash commands but says nothing about what they
//! do, and opman refuses to keep a table of prose for them: a table only ever describes the
//! commands its author happened to know, so every project command, skill and plugin the
//! user actually installed would go unlabelled while a stale built-in kept its caption.
//!
//! The descriptions are read from the same files claude reads. A command is a markdown file
//! under a `commands/` directory; a skill is a `SKILL.md`; both carry a `description:` in
//! their YAML frontmatter. Anything claude reports that has no file — its true built-ins —
//! has no description, and the UI shows the name alone.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

/// How deep a walk may go below one root. Deep enough for
/// `plugins/cache/<marketplace>/<plugin>/<version>/skills/<skill>/SKILL.md`, shallow enough
/// that a stray symlink into a source tree cannot turn discovery into a full filesystem scan.
const MAX_DEPTH: usize = 7;

/// Bytes of a file that may be frontmatter. A `description:` further in is not frontmatter.
const FRONTMATTER_LIMIT: u64 = 8 * 1024;

/// Descriptions for every command definition visible from `dir`, keyed by the name claude
/// would report for it.
///
/// Both `.claude` roots are read, project last so a project command shadows the user one of
/// the same name — the precedence claude itself applies.
pub fn describe(dir: &str) -> HashMap<String, String> {
    let mut found = HashMap::new();
    for root in roots(dir) {
        collect(&root, 0, &mut found);
    }
    found
}

/// Look a reported command name up in a description index.
///
/// Plugins namespace their commands (`hookify:hookify`), and the definition on disk is keyed
/// by its own bare name, so a miss retries on the segment after the last colon.
pub fn lookup<'a>(index: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    index
        .get(name)
        .or_else(|| index.get(name.rsplit(':').next()?))
        .map(String::as_str)
}

/// Directories that may hold command definitions, lowest precedence first.
fn roots(dir: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".claude/plugins"));
        roots.push(home.join(".claude/commands"));
        roots.push(home.join(".claude/skills"));
    }
    let project = Path::new(dir).join(".claude");
    roots.push(project.join("commands"));
    roots.push(project.join("skills"));
    roots
}

/// Walk one root, recording every definition file it contains.
fn collect(path: &Path, depth: usize, found: &mut HashMap<String, String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = path.read_dir() else {
        return;
    };
    for entry in entries.flatten() {
        // `file_type` on the entry, not on the path: it does not follow symlinks, so a link
        // pointing back up its own tree cannot be walked into a cycle.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let child = entry.path();
        if kind.is_dir() {
            collect(&child, depth + 1, found);
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let Some(name) = command_name(&child) else {
            continue;
        };
        let Some(description) = description_of(&child) else {
            continue;
        };
        found.insert(name, description);
    }
}

/// The command name a definition file answers to, or `None` if it is not one.
///
/// A `SKILL.md` is named by the directory holding it. A markdown file under a `commands/`
/// directory is named by its path below that directory, with claude's `:` separator for
/// nesting — `commands/git/sync.md` is `/git:sync`.
fn command_name(file: &Path) -> Option<String> {
    let stem = file.file_name()?.to_str()?;
    if stem.eq_ignore_ascii_case("SKILL.md") {
        return Some(file.parent()?.file_name()?.to_str()?.to_string());
    }
    if !stem.ends_with(".md") {
        return None;
    }
    let mut segments = vec![stem.trim_end_matches(".md")];
    let mut parent = file.parent();
    while let Some(directory) = parent {
        let label = directory.file_name().and_then(|n| n.to_str())?;
        if label == "commands" {
            segments.reverse();
            return Some(segments.join(":"));
        }
        segments.push(label);
        parent = directory.parent();
    }
    None
}

/// The `description:` from a file's YAML frontmatter.
///
/// Read line by line and abandoned at the closing fence: a definition file is mostly prose,
/// and none of it past the frontmatter is worth pulling into memory.
fn description_of(file: &Path) -> Option<String> {
    let handle = File::open(file).ok()?;
    let mut reader = BufReader::new(handle.take(FRONTMATTER_LIMIT));
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 || line.trim_end() != "---" {
        return None;
    }
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line.trim();
        if trimmed == "---" {
            return None;
        }
        if let Some(value) = trimmed.strip_prefix("description:") {
            let value = value.trim().trim_matches(['"', '\'']).trim();
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
}

#[cfg(test)]
#[path = "command_meta_tests.rs"]
mod command_meta_tests;
