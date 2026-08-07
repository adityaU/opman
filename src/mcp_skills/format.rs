//! Reading and writing `SKILL.md`.
//!
//! One renderer and one parser, where there used to be three copies of a `format!`
//! template and a parser that disagreed with all of them. The old template interpolated
//! the description straight into YAML, so a description containing `:` or a newline
//! produced frontmatter the parser then rejected — the write reported success and the
//! skill silently never appeared in any listing.

use serde::{Deserialize, Serialize};

use super::name::SkillName;
use super::Skill;

const DELIMITER: &str = "---";

/// Frontmatter, as written and as read. Going through serde in both directions is what
/// makes the round trip safe for arbitrary text.
#[derive(Debug, Default, Deserialize, Serialize)]
struct Frontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    /// Servers from `mcp.json` this skill needs. Accepts a list or a bare string.
    #[serde(default, alias = "requires_mcp", alias = "requiresMcp")]
    requires: Requires,
}

/// Lenient on the way in: `requires: jira` and `requires: [jira]` both work, because a
/// user writing one skill should not have to remember which.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(untagged)]
enum Requires {
    #[default]
    Absent,
    One(String),
    Many(Vec<String>),
}

impl Requires {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Absent => Vec::new(),
            Self::One(one) => vec![one],
            Self::Many(many) => many,
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    NoFrontmatter,
    Yaml(serde_yaml::Error),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFrontmatter => f.write_str("SKILL.md must start with a `---` frontmatter block"),
            Self::Yaml(e) => write!(f, "invalid SKILL.md frontmatter: {e}"),
        }
    }
}

/// Parse one `SKILL.md`.
///
/// The file must *start* with the delimiter. The previous parser split on `---`
/// anywhere, so a body containing a horizontal rule was read as frontmatter. The
/// directory name is authoritative for identity — the frontmatter `name` is only a
/// display title, because keying by it meant `skills delete` could not find a skill that
/// `skills list` had just printed.
pub fn parse_skill_md(raw: &str, dir_name: &SkillName) -> Result<Skill, ParseError> {
    let body = raw
        .strip_prefix(DELIMITER)
        .and_then(|rest| rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n")))
        .ok_or(ParseError::NoFrontmatter)?;
    let (front, content) = split_at_close(body).ok_or(ParseError::NoFrontmatter)?;
    let front: Frontmatter = serde_yaml::from_str(front).map_err(ParseError::Yaml)?;
    let title = if front.name.trim().is_empty() {
        dir_name.to_string()
    } else {
        front.name
    };
    Ok(Skill {
        name: dir_name.clone(),
        title,
        description: front.description,
        content: content.trim().to_string(),
        requires: front.requires.into_vec(),
    })
}

/// Split at the first closing delimiter that sits on its own line.
fn split_at_close(body: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line.trim_end() == DELIMITER {
            return Some((&body[..offset], &body[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// What to write. Borrowed so rendering never clones the body.
pub struct SkillDraft<'a> {
    pub name: &'a SkillName,
    pub title: Option<&'a str>,
    pub description: &'a str,
    pub requires: &'a [String],
    pub body: &'a str,
}

/// Render a `SKILL.md`. Frontmatter goes through serde_yaml, so any description round
/// trips — including one containing `:`, `#`, a newline, or a leading `-`.
pub fn render_skill_md(draft: &SkillDraft<'_>) -> Result<String, serde_yaml::Error> {
    let front = Frontmatter {
        name: draft.title.unwrap_or(draft.name.as_str()).to_string(),
        description: draft.description.to_string(),
        requires: match draft.requires {
            [] => Requires::Absent,
            many => Requires::Many(many.to_vec()),
        },
    };
    let yaml = serde_yaml::to_string(&front)?;
    Ok(format!("{DELIMITER}\n{yaml}{DELIMITER}\n\n{}\n", draft.body.trim()))
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod format_tests;
