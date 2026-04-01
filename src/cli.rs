use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::web::TunnelMode;

/// Parse a "truthy" string into a bool.
/// Accepts: `1`, `true`, `yes`, `on`, `quick` → `true`.
/// Accepts: `0`, `false`, `no`, `off` → `false`.
fn parse_truthy(s: &str) -> Result<bool, String> {
    match s.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "quick" => Ok(true),
        "0" | "false" | "no" | "off" | "" => Ok(false),
        other => Err(format!(
            "invalid value '{other}' (expected: true/false/1/0/yes/no/on/off)"
        )),
    }
}

#[derive(Parser)]
#[command(
    name = "opman",
    version,
    about = "Terminal multiplexer wrapper for the opencode CLI — multi-project management",
    long_about = "opman is a terminal UI that wraps the opencode CLI, providing\n\
                  multi-project management, a web UI, Cloudflare tunnel support,\n\
                  MCP tool integrations, and more."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    // ── Web UI ──────────────────────────────────────────────────────
    /// Enable the web UI server
    #[arg(long)]
    pub web: bool,

    /// Run in web-only mode (no TUI)
    #[arg(long)]
    pub web_only: bool,

    /// Port for the web UI server (default: random available port)
    #[arg(long, value_name = "PORT")]
    pub web_port: Option<u16>,

    /// Username for web UI authentication
    #[arg(long, value_name = "USER", env = "OPMAN_WEB_USER")]
    pub web_user: Option<String>,

    /// Password for web UI authentication
    #[arg(long, value_name = "PASS", env = "OPMAN_WEB_PASS")]
    pub web_pass: Option<String>,

    // ── Cloudflare Tunnel ───────────────────────────────────────────
    /// Enable Cloudflare quick tunnel (ephemeral trycloudflare.com URL).
    /// Requires --web-user and --web-pass.
    #[arg(
        long,
        env = "OPMAN_CF_TUNNEL",
        default_missing_value = "true",
        num_args = 0..=1,
        value_parser = parse_truthy,
    )]
    pub tunnel: bool,

    /// Cloudflare named tunnel token (remote-managed). Enables a persistent
    /// tunnel using `cloudflared tunnel run --token <TOKEN>`.
    /// Ingress must be configured in the Cloudflare dashboard.
    /// Requires --web-user and --web-pass.
    #[arg(long, value_name = "TOKEN", env = "OPMAN_CF_TUNNEL_TOKEN")]
    pub tunnel_token: Option<String>,

    /// External hostname for a locally-managed Cloudflare tunnel
    /// (e.g. "opman.example.com"). On first run, opens a browser for
    /// Cloudflare login. The tunnel, DNS record, and ingress config are
    /// created and managed automatically. Requires --web-user and --web-pass.
    #[arg(long, value_name = "HOSTNAME", env = "OPMAN_CF_TUNNEL_HOSTNAME")]
    pub tunnel_hostname: Option<String>,

    /// Name for the locally-managed tunnel (default: "opman").
    /// Only used with --tunnel-hostname.
    #[arg(
        long,
        value_name = "NAME",
        env = "OPMAN_CF_TUNNEL_NAME",
        default_value = "opman"
    )]
    pub tunnel_name: String,

    /// Force cloudflared to use a specific protocol (e.g. "http2").
    /// Useful on networks where SRV DNS lookups for QUIC are blocked
    /// (e.g. corporate-managed machines).
    #[arg(long, value_name = "PROTO", env = "OPMAN_CF_TUNNEL_PROTOCOL")]
    pub tunnel_protocol: Option<String>,

    /// Force cloudflared to connect to a specific region (e.g. "us").
    /// Can help on corporate networks that block Cloudflare edge DNS.
    #[arg(long, value_name = "REGION", env = "OPMAN_CF_TUNNEL_REGION")]
    pub tunnel_region: Option<String>,

    /// Hardcode Cloudflare edge IP:port addresses, bypassing DNS discovery
    /// entirely.  Accepts one or more addresses (comma-separated or repeated).
    /// Example: --tunnel-edge-ip 198.41.192.167:7844 --tunnel-edge-ip 198.41.200.13:7844
    /// This is the nuclear option for corporate networks that block all
    /// Cloudflare DNS lookups.
    #[arg(
        long,
        value_name = "ADDR",
        env = "OPMAN_CF_TUNNEL_EDGE_IP",
        value_delimiter = ','
    )]
    pub tunnel_edge_ip: Vec<String>,

    // ── MCP control ─────────────────────────────────────────────────
    /// Enable all MCP integrations (terminal, neovim, time, ui)
    #[arg(long)]
    pub all_mcp: bool,

    /// Enable the terminal MCP server
    #[arg(long)]
    pub terminal_mcp: bool,

    /// Enable the neovim MCP server
    #[arg(long)]
    pub neovim_mcp: bool,

    /// Enable the time MCP server
    #[arg(long)]
    pub time_mcp: bool,

    /// Enable the UI render MCP server (A2UI)
    #[arg(long)]
    pub ui_mcp: bool,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Run the MCP stdio bridge for a project
    Mcp {
        /// Path to the project directory
        project_path: PathBuf,
    },

    /// Run the time MCP bridge
    McpTime,

    /// Run the UI render MCP bridge (A2UI)
    McpUi,

    /// Run the neovim MCP bridge for a project
    McpNvim {
        /// Path to the project directory
        project_path: PathBuf,
    },

    /// Print the Slack app manifest and copy to clipboard
    SlackManifest,

    /// Manage skills
    Skills {
        #[command(subcommand)]
        subcommand: SkillsCommands,
    },
}

#[derive(Subcommand)]
pub(crate) enum SkillsCommands {
    /// List all skills
    List,

    /// Create a new skill
    Create {
        /// Skill name
        name: String,
        /// Skill description
        description: String,
        /// Skill content
        content: String,
    },

    /// Update an existing skill
    Update {
        /// Skill name
        name: String,
        /// New description
        description: String,
        /// New content
        content: String,
    },

    /// Delete a skill
    Delete {
        /// Skill name
        name: String,
    },

    /// Show a skill
    Show {
        /// Skill name
        name: String,
    },
}

impl Cli {
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

    /// Whether the web server should be enabled.
    pub fn enable_web(&self) -> bool {
        self.web_only
            || self.web
            || self.web_port.is_some()
            || self.web_user.as_deref().map_or(false, |u| !u.is_empty())
            || self.tunnel_mode().is_some()
    }

    /// Validate CLI argument combinations. Returns an error message on failure.
    pub fn validate(&self) -> Result<(), String> {
        let tunnel = self.tunnel_mode();

        // Tunnel requires authentication
        if tunnel.is_some() {
            let has_user = self.web_user.as_deref().map_or(false, |u| !u.is_empty());
            let has_pass = self.web_pass.as_deref().map_or(false, |p| !p.is_empty());
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
