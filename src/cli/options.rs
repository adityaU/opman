use super::{AgentBackend, Cli};
use crate::web::TunnelMode;

impl Cli {
    /// Say so when a launch still passes one of the old MCP flags. They do nothing now:
    /// which of opman's own MCP servers are on is decided in Settings.
    pub fn warn_deprecated_mcp_flags(&self) {
        let given = [
            ("--all-mcp", self.all_mcp),
            ("--terminal-mcp", self.terminal_mcp),
            ("--neovim-mcp", self.neovim_mcp),
            ("--time-mcp", self.time_mcp),
            ("--ui-mcp", self.ui_mcp),
            ("--code-mcp", self.code_mcp),
        ];
        for (flag, set) in given {
            if set {
                tracing::warn!("{flag} is deprecated and ignored; turn MCP servers on and off in Settings");
            }
        }
    }

    /// Determine the tunnel mode from CLI flags and env vars.
    ///
    /// Priority: `--tunnel-hostname` > `--tunnel-token` > `--tunnel`.
    /// Env vars (`OPMAN_CF_TUNNEL_HOSTNAME`, `OPMAN_CF_TUNNEL_TOKEN`,
    /// `OPMAN_CF_TUNNEL`) are handled by clap's `env` attribute.
    pub fn tunnel_mode(&self) -> Option<TunnelMode> {
        // Local-managed tunnel: user provides hostname, we handle everything
        if let Some(ref hostname) = self.tunnel_hostname {
            let hostname = hostname.trim().to_string();
            if !hostname.is_empty() {
                return Some(TunnelMode::LocalManaged {
                    hostname,
                    tunnel_name: self.tunnel_name.clone(),
                });
            }
        }
        // Remote-managed tunnel: user provides dashboard token
        if let Some(ref token) = self.tunnel_token {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some(TunnelMode::Named { token });
            }
        }
        if self.tunnel {
            return Some(TunnelMode::Quick);
        }
        None
    }

    /// The effective backend. `--acp` wins over `--claude`, which wins over
    /// `--backend`.
    pub fn resolved_backend(&self) -> AgentBackend {
        if self.acp {
            AgentBackend::ClaudeAcp
        } else if self.claude {
            AgentBackend::ClaudeCode
        } else {
            self.backend
        }
    }

    /// Whether the web server should be enabled.
    /// Which browser backs the browser panes.
    pub fn browser_mode(&self) -> crate::browser::BrowserMode {
        crate::browser::BrowserMode::from_device_flag(self.device_browser.into())
    }

    pub fn enable_web(&self) -> bool {
        self.web_only
            || self.web
            || self.web_port.is_some()
            || self.web_user.as_deref().is_some_and(|u| !u.is_empty())
            || self.tunnel_mode().is_some()
    }

    /// Validate CLI argument combinations. Returns an error message on failure.
    pub fn validate(&self) -> Result<(), String> {
        let tunnel = self.tunnel_mode();

        // Tunnel requires authentication
        if tunnel.is_some() {
            let has_user = self.web_user.as_deref().is_some_and(|u| !u.is_empty());
            let has_pass = self.web_pass.as_deref().is_some_and(|p| !p.is_empty());
            if !has_user || !has_pass {
                return Err("Cloudflare tunnel requires authentication.\n\
                     Provide --web-user and --web-pass (or OPMAN_WEB_USER / OPMAN_WEB_PASS)\n\
                     to secure the web UI before exposing it via tunnel."
                    .to_string());
            }
        }

        // web-only implies web
        if self.web_only && !self.enable_web() {
            // This can't really happen since web_only => enable_web(), but just in case.
            return Err("--web-only requires the web server to be enabled.".to_string());
        }

        Ok(())
    }
}
