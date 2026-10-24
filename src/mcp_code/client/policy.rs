//! Live code-mode settings for an already running code-mode pool.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ServerPolicy {
    pub enabled: bool,
    pub disabled_tools: HashSet<String>,
}

pub(super) type Provider = Arc<dyn Fn() -> HashMap<String, ServerPolicy> + Send + Sync>;

pub(super) fn settings_provider() -> Provider {
    Arc::new(|| {
        crate::mcp_registry::config::load()
            .servers
            .into_iter()
            .map(|(name, config)| {
                (
                    name,
                    ServerPolicy {
                        enabled: config.code_mode.unwrap_or(true),
                        disabled_tools: config.code_mode_disabled_tools.into_iter().collect(),
                    },
                )
            })
            .collect()
    })
}

pub(super) fn server_enabled(policies: &HashMap<String, ServerPolicy>, server: &str) -> bool {
    policies.get(server).map_or_else(
        || crate::mcp_registry::builtin::is_builtin(server),
        |policy| policy.enabled,
    )
}
