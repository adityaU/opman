//! `opman skills …`.
//!
//! Lifted out of `main.rs`, and every path now goes through [`SkillName`]: `opman skills
//! delete ../..` used to reach `std::fs::remove_dir_all` on whatever the user typed.

use anyhow::{bail, Result};

use crate::cli::SkillsCommands;
use crate::mcp_skills::format::{render_skill_md, SkillDraft};
use crate::mcp_skills::{get_skills_dir, load_skills, SkillName};

pub(crate) async fn handle_skills(subcommand: SkillsCommands) -> Result<()> {
    match subcommand {
        SkillsCommands::List => {
            for (name, skill) in load_skills().await? {
                println!("{name}: {}", skill.description);
            }
        }
        SkillsCommands::Create {
            name,
            description,
            content,
        } => {
            let name = parse(&name)?;
            let dir = name.dir_in(&get_skills_dir());
            std::fs::create_dir_all(&dir)?;
            write(&dir, &name, &description, &content)?;
            println!("Skill '{name}' created.");
        }
        SkillsCommands::Update {
            name,
            description,
            content,
        } => {
            let name = parse(&name)?;
            let dir = name.dir_in(&get_skills_dir());
            if !dir.is_dir() {
                bail!("Skill '{name}' not found");
            }
            write(&dir, &name, &description, &content)?;
            println!("Skill '{name}' updated.");
        }
        SkillsCommands::Delete { name } => {
            let name = parse(&name)?;
            let dir = name.dir_in(&get_skills_dir());
            if !dir.is_dir() {
                bail!("Skill '{name}' not found");
            }
            std::fs::remove_dir_all(&dir)?;
            println!("Skill '{name}' deleted.");
        }
        SkillsCommands::Show { name } => {
            let name = parse(&name)?;
            let registry = load_skills().await?;
            let Some(skill) = registry.get(&name) else {
                bail!("Skill '{name}' not found");
            };
            println!("Name: {}", skill.name);
            println!("Title: {}", skill.title);
            println!("Description: {}", skill.description);
            if !skill.requires.is_empty() {
                println!("Requires: {}", skill.requires.join(", "));
            }
            println!("Content:\n{}", skill.content);
        }
    }
    Ok(())
}

fn parse(raw: &str) -> Result<SkillName> {
    SkillName::parse(raw).map_err(|error| anyhow::anyhow!("invalid skill name '{raw}': {error}"))
}

fn write(dir: &std::path::Path, name: &SkillName, description: &str, body: &str) -> Result<()> {
    let rendered = render_skill_md(&SkillDraft {
        name,
        title: None,
        description,
        requires: &[],
        body,
    })?;
    std::fs::write(dir.join("SKILL.md"), rendered)?;
    Ok(())
}

#[cfg(test)]
#[path = "cli_skills_tests.rs"]
mod cli_skills_tests;
