//! Environment-isolated MCP configuration helper for web tests.

pub(crate) struct ConfigRedirect {
    _tmp: tempfile::TempDir,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl ConfigRedirect {
    pub(crate) fn new() -> Self {
        let guard = crate::claude_engine::claude_cli::ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let vars = [
            ("OPMAN_MCP_CONFIG", tmp.path().join("mcp.json")),
            ("XDG_CONFIG_HOME", tmp.path().to_path_buf()),
        ];
        let previous = vars
            .iter()
            .map(|(key, value)| {
                let prior = std::env::var_os(key);
                std::env::set_var(key, value);
                (*key, prior)
            })
            .collect();
        Self {
            _tmp: tmp,
            previous,
            _guard: guard,
        }
    }

    pub(crate) fn document(&self) -> crate::mcp_registry::config::McpConfig {
        crate::mcp_registry::config::load()
    }

    pub(crate) fn declare(&self, name: &str, entry: crate::mcp_registry::config::ServerConfig) {
        let mut document = self.document();
        document.servers.insert(name.to_string(), entry);
        crate::mcp_registry::config::save(&document).expect("save mcp.json");
    }
}

impl Drop for ConfigRedirect {
    fn drop(&mut self) {
        for (key, prior) in self.previous.drain(..) {
            match prior {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
