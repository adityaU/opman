//! `opman update` and `opman downgrade`: replace this binary with another published release.

mod install;
mod release;
mod version;

use anyhow::{bail, Context, Result};

use version::Target;
pub(crate) use version::Version;

/// What `update` should do given the newest release.
#[derive(Debug, PartialEq, Eq)]
enum UpdatePlan {
    UpToDate,
    Install(Version),
}

fn plan_update(current: Version, latest: Version) -> UpdatePlan {
    if latest > current {
        UpdatePlan::Install(latest)
    } else {
        UpdatePlan::UpToDate
    }
}

/// `downgrade` only goes backwards; going forwards is `update`'s job.
fn check_downgrade(current: Version, wanted: Version) -> Result<()> {
    if wanted >= current {
        bail!(
            "{} is not older than the installed {}. Use `opman update --version {wanted}` instead.",
            wanted.tag(),
            current.tag()
        );
    }
    Ok(())
}

fn current() -> Result<Version> {
    Version::current().context("this build has no valid version number")
}

/// The newest release if it is newer than this binary, for `opman doctor`. Gives up after a few seconds.
pub(crate) async fn newer_release() -> Result<Option<Version>> {
    const WAIT: std::time::Duration = std::time::Duration::from_secs(5);
    let current = current()?;
    let client = release::client()?;
    let latest = tokio::time::timeout(WAIT, release::latest(&client))
        .await
        .context("GitHub did not answer in time")??;
    Ok(match plan_update(current, latest) {
        UpdatePlan::Install(version) => Some(version),
        UpdatePlan::UpToDate => None,
    })
}

/// `opman update [--version V]`. With a version it installs exactly that one, any direction.
pub(crate) async fn update(requested: Option<Version>) -> Result<()> {
    let current = current()?;
    let client = release::client()?;
    let wanted = match requested {
        Some(version) if version == current => {
            println!("Already on {}.", current.tag());
            return Ok(());
        }
        Some(version) => version,
        None => match plan_update(current, release::latest(&client).await?) {
            UpdatePlan::UpToDate => {
                println!("opman {} is the latest release.", current.tag());
                return Ok(());
            }
            UpdatePlan::Install(version) => version,
        },
    };
    install(&client, current, wanted).await
}

/// `opman downgrade [VERSION]`. Without a version it lists what could be installed.
pub(crate) async fn downgrade(requested: Option<Version>) -> Result<()> {
    let current = current()?;
    let client = release::client()?;
    let Some(wanted) = requested else {
        let older: Vec<Version> = release::list(&client)
            .await?
            .into_iter()
            .filter(|v| *v < current)
            .collect();
        println!("Installed: {}", current.tag());
        if older.is_empty() {
            println!("No older releases are published.");
            return Ok(());
        }
        println!("Older releases (run `opman downgrade <version>`):");
        older.iter().for_each(|v| println!("  {}", v.tag()));
        return Ok(());
    };
    check_downgrade(current, wanted)?;
    install(&client, current, wanted).await
}

async fn install(client: &reqwest::Client, current: Version, wanted: Version) -> Result<()> {
    let target = Target::detect().context("no prebuilt binary exists for this platform")?;
    let exe = std::env::current_exe().context("could not find the running opman binary")?;
    println!("Installing {} (was {})...", wanted.tag(), current.tag());
    tracing::debug!(exe = %exe.display(), ?target, "starting self-update");

    let staging = install::Staging::beside(&exe)?;
    release::download(client, wanted, target, &staging.archive).await?;
    let staging = tokio::task::spawn_blocking(move || {
        install::extract_binary(&staging.archive, &staging.binary)?;
        Ok::<_, anyhow::Error>(staging)
    })
    .await
    .context("unpacking was interrupted")??;
    install::replace(&exe, &staging.binary)?;

    println!(
        "Now on {}. Restart any running opman to use it.",
        wanted.tag()
    );
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
