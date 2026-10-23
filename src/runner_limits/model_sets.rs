//! Named groups of equivalent models, using the model ids from each runner's catalogue.

use std::collections::HashSet;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use super::settings::{FallbackPair, LimitSettings, ModelRef};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSet {
    pub id: String,
    pub name: String,
    pub members: Vec<FallbackPair>,
}

impl LimitSettings {
    pub fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut models = HashSet::new();
        for set in &self.model_sets {
            if set.id.trim().is_empty() || set.name.trim().is_empty() {
                bail!("Model sets need an id and a name");
            }
            if !ids.insert(&set.id) || !names.insert(set.name.trim()) {
                bail!("Model set ids and names must be unique");
            }
            if set.members.len() < 2 {
                bail!("Model set '{}' needs at least two models", set.name);
            }
            let mut runners = HashSet::new();
            for member in &set.members {
                let Some(model) = member.model.as_ref() else {
                    bail!("Every member of '{}' needs a model", set.name);
                };
                if member.runner.trim().is_empty()
                    || model.provider_id.trim().is_empty()
                    || model.model_id.trim().is_empty()
                {
                    bail!("Model set members need a runner, provider and model id");
                }
                if !models.insert((&member.runner, &model.provider_id, &model.model_id)) {
                    bail!("A runner model can belong to only one model set");
                }
                if !runners.insert(&member.runner) {
                    bail!(
                        "Model set '{}' can have only one model per runner",
                        set.name
                    );
                }
            }
            if runners.len() < 2 {
                bail!(
                    "Model set '{}' needs models from different runners",
                    set.name
                );
            }
        }
        Ok(())
    }

    /// A matching set limits the choice to its members. No match keeps legacy pairs.
    pub fn candidates(&self, runner: &str, model: Option<&ModelRef>) -> &[FallbackPair] {
        let Some(model) = model else {
            return &self.pairs;
        };
        let mut matching = self.model_sets.iter().filter(|set| {
            set.members.iter().any(|member| {
                member.runner == runner
                    && member.model.as_ref().is_some_and(|candidate| {
                        candidate.model_id == model.model_id
                            && (model.provider_id.is_empty()
                                || candidate.provider_id == model.provider_id)
                    })
            })
        });
        let Some(set) = matching.next() else {
            return &self.pairs;
        };
        // Older ACP transcripts contain only a model id. Never guess between providers.
        if matching.next().is_some() {
            return &self.pairs;
        }
        &set.members
    }
}

#[cfg(test)]
#[path = "model_sets_tests.rs"]
mod tests;
