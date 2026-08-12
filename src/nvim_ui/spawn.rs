//! Start one embedded Neovim process and derive its private listen socket.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::{Child, Command};

use super::error::{NvimUiError, Result};
use super::key::SessionKey;

pub struct SpawnedNvim {
    pub child: Child,
    pub socket_path: PathBuf,
    pub stdout: tokio::process::ChildStdout,
    pub stdin: tokio::process::ChildStdin,
}

#[derive(Clone, Debug)]
pub enum ConfigSource {
    UserConfig,
    Minimal {
        init: PathBuf,
        app_name: String,
        config_home: PathBuf,
    },
}

pub async fn spawn(
    project_dir: &Path,
    key: &SessionKey,
    config: &ConfigSource,
) -> Result<SpawnedNvim> {
    let socket_path = socket_path(key);
    remove_stale_socket(&socket_path)?;
    let runtime_dir = socket_path
        .parent()
        .ok_or_else(|| NvimUiError::Spawn(std::io::Error::other("invalid runtime directory")))?;
    std::fs::create_dir_all(runtime_dir).map_err(NvimUiError::Spawn)?;

    let mut command = Command::new("nvim");
    command
        .arg("--embed")
        .arg("--listen")
        .arg(&socket_path)
        .arg("--cmd")
        .arg("set noswapfile")
        .arg("--cmd")
        .arg("set shortmess+=A");
    if let ConfigSource::Minimal {
        init,
        app_name,
        config_home,
    } = config
    {
        command
            .arg("-u")
            .arg(init)
            .env("NVIM_APPNAME", app_name)
            .env("XDG_CONFIG_HOME", config_home)
            .env("XDG_DATA_HOME", config_home.join("data"))
            .env("XDG_STATE_HOME", config_home.join("state"))
            .env("XDG_CACHE_HOME", config_home.join("cache"));
    }
    let mut child = command
        .current_dir(project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(NvimUiError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(NvimUiError::MissingPipe("stdout"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or(NvimUiError::MissingPipe("stdin"))?;
    Ok(SpawnedNvim {
        child,
        socket_path,
        stdout,
        stdin,
    })
}

pub fn socket_path(key: &SessionKey) -> PathBuf {
    let runtime = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    runtime.join(format!("nvim-ui-{:x}.sock", hasher.finish()))
}

fn remove_stale_socket(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(NvimUiError::Spawn(error)),
    }
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod spawn_tests;
