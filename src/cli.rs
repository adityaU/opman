use clap::{Parser, ValueEnum};

mod commands;
mod options;
pub(crate) use commands::{Commands, SkillsCommands};

/// Parse a "truthy" string into a bool.
/// Accepts: `1`, `true`, `yes`, `on`, `quick` → `true`.
/// Accepts: `0`, `false`, `no`, `off` → `false`.
/// Which agent backend to spawn and proxy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum AgentBackend {
    /// Use the `opencode` CLI (default).
    #[default]
    Opencode,
    /// Use the `claude` CLI background agents (`claude --bg`).
    ClaudeCode,
    /// Drive the `claude` CLI over the Agent Client Protocol, via the generic ACP
    /// engine. One ACP server per session, with per-token streaming, protocol-level
    /// permission prompts, and cooperative cancellation.
    ClaudeAcp,
}

impl AgentBackend {
    /// The binary name for this backend.
    pub fn binary(&self) -> &'static str {
        match self {
            AgentBackend::Opencode => "opencode",
            AgentBackend::ClaudeCode | AgentBackend::ClaudeAcp => "claude",
        }
    }

    /// Short display name used in the web UI status bar.
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentBackend::Opencode => "opencode",
            // Both claude engines present as "claude-code" to the web UI.
            AgentBackend::ClaudeCode | AgentBackend::ClaudeAcp => "claude-code",
        }
    }
}

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

    /// Write debug-level logs to ~/.config/opman/opman.log (beats RUST_LOG)
    #[arg(long, global = true, env = "OPMAN_DEBUG", value_parser = clap::builder::BoolishValueParser::new())]
    pub debug: bool,

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

    /// Address the web UI server listens on. Use 127.0.0.1 to keep it off the network.
    #[arg(
        long,
        value_name = "IP",
        env = "OPMAN_WEB_BIND",
        default_value = "0.0.0.0"
    )]
    pub web_bind: std::net::IpAddr,

    /// Stop web-only mode when stdin closes. A parent that pipes stdin and then
    /// dies (even when killed) takes opman down with it.
    #[arg(long, requires = "web_only")]
    pub exit_on_stdin_eof: bool,

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
    /// Which agent backend to use (opencode or claude-code).
    /// Default: opencode.
    #[arg(
        long,
        value_name = "BACKEND",
        env = "OPMAN_BACKEND",
        default_value = "opencode"
    )]
    pub backend: AgentBackend,

    /// Shorthand for `--backend claude-code`: use the Claude Code CLI as the
    /// agent engine (an in-process adapter drives `claude` background sessions).
    #[arg(long)]
    pub claude: bool,

    /// Drive Claude over the Agent Client Protocol (the generic ACP engine). Unlike
    /// `--claude`, replies stream per token, permission prompts are protocol requests
    /// rather than hook callbacks, and abort cancels the turn instead of killing the
    /// process. Takes precedence over `--claude`.
    ///
    /// `--claudep` is kept as an alias: this replaced the `claude -p` engine that flag
    /// used to select.
    #[arg(long = "acp", alias = "claudep")]
    pub acp: bool,

    /// Deprecated and ignored: MCP servers are switched on and off in Settings.
    #[arg(long, hide = true)]
    pub all_mcp: bool,

    /// Deprecated and ignored: MCP servers are switched on and off in Settings.
    #[arg(long, hide = true)]
    pub terminal_mcp: bool,

    /// Deprecated and ignored: MCP servers are switched on and off in Settings.
    #[arg(long, hide = true)]
    pub neovim_mcp: bool,

    /// Deprecated and ignored: MCP servers are switched on and off in Settings.
    #[arg(long, hide = true)]
    pub time_mcp: bool,

    /// Deprecated and ignored: MCP servers are switched on and off in Settings.
    #[arg(long, hide = true)]
    pub ui_mcp: bool,

    /// Deprecated and ignored: MCP servers are switched on and off in Settings.
    #[arg(long, hide = true)]
    pub code_mcp: bool,

    // ── Browser panes ───────────────────────────────────────────────
    /// Drive this device's own browser (Chrome → Edge → Brave → Chromium), headed on the
    /// real display with its windows kept off-screen, instead of a private Chromium on a
    /// virtual display. The desktop app passes this. Remote servers can borrow it through
    /// a device link.
    #[arg(
        long,
        env = "OPMAN_DEVICE_BROWSER",
        default_missing_value = "true",
        num_args = 0..=1,
        value_parser = parse_truthy,
    )]
    pub device_browser: bool,
}
