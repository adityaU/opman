//! ACP agents published as a downloadable archive rather than a package or a binary on
//! `PATH` — Google Antigravity's `agy_acp_server` is one. The ACP registry lists a zip
//! per platform; opman fetches the one for this machine the first time the agent starts,
//! unpacks it under its data directory and launches it from there.
//!
//! The install is atomic: the zip is streamed to a temp file (it is hundreds of MB, so
//! never held in memory), unpacked into a `.partial` directory, and renamed into place
//! last. A crash mid-download therefore leaves nothing that looks installed, and the next
//! start simply tries again.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::config::AgentConfig;

/// One platform's download.
pub struct Build {
    /// `std::env::consts::OS` value.
    pub os: &'static str,
    /// `std::env::consts::ARCH` value.
    pub arch: &'static str,
    pub url: &'static str,
    /// The executable, relative to the unpacked archive.
    pub cmd: &'static str,
    /// Arguments the registry says this build needs.
    pub args: &'static [&'static str],
}

/// A versioned, per-platform archive.
pub struct Archive {
    pub version: &'static str,
    pub builds: &'static [Build],
}

impl Archive {
    /// The build for the machine opman runs on.
    pub fn build(&self) -> Option<&'static Build> {
        let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
        self.builds
            .iter()
            .find(|build| build.os == os && build.arch == arch)
    }

    /// Where this version unpacks: `<data dir>/opman/acp/<id>/<version>`.
    pub fn install_dir(&self, id: &str) -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join("opman").join("acp").join(id).join(self.version))
    }

    /// The command and arguments for this machine, or `None` when there is no build for
    /// it — which leaves the agent unlaunchable rather than pointed at a wrong binary.
    pub fn launch(&self, id: &str) -> Option<(String, Vec<String>)> {
        let build = self.build()?;
        let command = self.install_dir(id)?.join(build.cmd);
        let args = build.args.iter().map(|arg| (*arg).to_string()).collect();
        Some((command.to_string_lossy().into_owned(), args))
    }
}

/// One install at a time: two connections opening together must not both download.
static INSTALLING: Mutex<()> = Mutex::const_new(());

/// Make sure an archive-launched agent is on disk before it is spawned. A no-op for every
/// other agent, and for one whose command the user pointed somewhere else.
pub async fn ensure_installed(agent: &AgentConfig) -> Result<()> {
    let Some(archive) = super::catalog::archive_for(&agent.runner) else {
        return Ok(());
    };
    let (Some(build), Some(dir)) = (archive.build(), archive.install_dir(&agent.runner)) else {
        return Ok(());
    };
    let command = dir.join(build.cmd);
    if Path::new(&agent.command) != command || command.is_file() {
        return Ok(());
    }
    let _guard = INSTALLING.lock().await;
    if command.is_file() {
        return Ok(());
    }
    tracing::info!(
        "installing ACP agent {} {} from {}",
        agent.runner,
        archive.version,
        build.url
    );
    install(build.url, &dir)
        .await
        .with_context(|| format!("installing {} {}", agent.display_name, archive.version))
}

async fn install(url: &str, dir: &Path) -> Result<()> {
    let parent = dir.parent().context("install dir has no parent")?;
    tokio::fs::create_dir_all(parent).await?;
    let zip = dir.with_extension("zip.partial");
    let staging = dir.with_extension("partial");
    download(url, &zip).await?;
    let (from, to) = (zip.clone(), staging.clone());
    tokio::task::spawn_blocking(move || unpack(&from, &to)).await??;
    let _ = tokio::fs::remove_file(&zip).await;
    tokio::fs::rename(&staging, dir)
        .await
        .with_context(|| format!("moving into {}", dir.display()))
}

async fn download(url: &str, to: &Path) -> Result<()> {
    let mut response = reqwest::get(url).await?.error_for_status()?;
    let mut file = tokio::fs::File::create(to).await?;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

/// Unpack `zip` into a fresh `into`, keeping each entry's Unix permissions — the
/// executables are useless without their execute bit.
fn unpack(zip: &Path, into: &Path) -> Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into)?;
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip)?)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            bail!("archive entry {:?} escapes its folder", entry.name());
        };
        let target = into.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut out)?;
        set_mode(&target, entry.unix_mode())?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: Option<u32>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) = mode {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o777))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: Option<u32>) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod archive_tests;
