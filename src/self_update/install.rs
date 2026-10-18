//! Unpacking the downloaded archive and swapping it in for the running binary.

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;

const BINARY: &str = "opman";

/// Scratch files kept next to the binary so the final rename stays on one filesystem.
/// Removed on drop, whether the update worked or not.
pub(super) struct Staging {
    pub archive: PathBuf,
    pub binary: PathBuf,
    directory: PathBuf,
}

impl Staging {
    pub fn beside(exe: &Path) -> Result<Self> {
        let dir = exe
            .parent()
            .context("the opman binary has no parent directory")?;
        let directory = dir.join(format!(
            ".opman-update-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .context("could not create the update directory beside the binary")?;
        Ok(Self {
            archive: directory.join("archive.tar.gz"),
            binary: directory.join("opman"),
            directory,
        })
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        // Missing files are fine: a failed download never made them.
        let _ = std::fs::remove_file(&self.archive);
        let _ = std::fs::remove_file(&self.binary);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

/// Pull the `opman` binary out of the archive into `out`.
pub(super) fn extract_binary(archive: &Path, out: &Path) -> Result<()> {
    let file =
        File::open(archive).with_context(|| format!("could not open {}", archive.display()))?;
    let mut tar = tar::Archive::new(GzDecoder::new(file));
    for entry in tar.entries().context("the archive is not a valid tar.gz")? {
        let mut entry = entry.context("the archive is damaged")?;
        let is_binary = entry
            .path()
            .ok()
            .and_then(|p| p.file_name().map(|n| n == BINARY))
            .unwrap_or(false);
        if !is_binary {
            continue;
        }
        if !entry.header().entry_type().is_file() {
            bail!("the archive's '{BINARY}' entry is not a regular file");
        }
        let mut binary = File::create_new(out).context("could not create the new binary")?;
        std::io::copy(&mut entry, &mut binary).context("could not unpack the binary")?;
        binary.sync_all().context("could not save the new binary")?;
        make_executable(out)?;
        return Ok(());
    }
    bail!("the archive does not contain an '{BINARY}' binary")
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .context("could not mark the new binary executable")
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Swap the new binary in. A rename never leaves a half-written file, and a running opman keeps
/// its old inode until it restarts.
pub(super) fn replace(exe: &Path, new_binary: &Path) -> Result<()> {
    std::fs::rename(new_binary, exe).with_context(|| {
        format!(
            "could not replace {} (is the folder writable?)",
            exe.display()
        )
    })
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
