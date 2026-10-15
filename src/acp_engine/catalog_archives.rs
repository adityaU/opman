//! The download tables for catalogue agents that ship as archives (see
//! [`super::super::archive`]). Data only, like `catalog_entries.rs`.

use super::super::archive::{Archive, Build};

/// Google Antigravity's ACP server, as the ACP registry publishes it
/// (`cdn.agentclientprotocol.com/registry/v1/latest/registry.json`, id `antigravity-acp`).
/// Linux builds take an empty `--uid=`, exactly as the registry lists them.
pub static ANTIGRAVITY: Archive = Archive {
    version: "1.4.0",
    builds: &[
        Build { os: "macos", arch: "aarch64", url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.4.0-darwin-arm64.zip", cmd: "agy_acp_server.par", args: &[] },
        Build { os: "macos", arch: "x86_64", url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.4.0-darwin-x86_64.zip", cmd: "agy_acp_server.par", args: &[] },
        Build { os: "linux", arch: "x86_64", url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.4.0-linux-x86_64.zip", cmd: "agy_acp_server.par", args: &["--uid="] },
        Build { os: "linux", arch: "aarch64", url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.4.0-linux-arm64.zip", cmd: "agy_acp_server.par", args: &["--uid="] },
        Build { os: "windows", arch: "x86_64", url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-1.4.0-windows-x86_64.zip", cmd: "agy_acp_server.exe", args: &[] },
        Build { os: "windows", arch: "aarch64", url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-1.4.0-windows-arm64.zip", cmd: "agy_acp_server.exe", args: &[] },
    ],
};
