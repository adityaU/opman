//! Talking to GitHub releases: which versions exist, and downloading one binary archive.

use std::path::Path;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::version::{Target, Version};

const REPO: &str = "adityaU/opman";
const LIST_PAGE: u8 = 50;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

impl Release {
    fn version(&self) -> Option<Version> {
        if self.draft || self.prerelease {
            return None;
        }
        self.tag_name.parse().ok()
    }
}

pub(super) fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("opman/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .context("could not create the HTTP client")
}

/// The newest published release.
pub(super) async fn latest(client: &reqwest::Client) -> Result<Version> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    tracing::debug!(%url, "fetching latest release");
    let release: Release = client
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("could not ask GitHub for the latest release")?
        .json()
        .await
        .context("GitHub sent a release we could not read")?;
    release.tag_name.parse().with_context(|| {
        format!(
            "latest release has an unexpected tag '{}'",
            release.tag_name
        )
    })
}

/// Published releases, newest first.
pub(super) async fn list(client: &reqwest::Client) -> Result<Vec<Version>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page={LIST_PAGE}");
    tracing::debug!(%url, "listing releases");
    let releases: Vec<Release> = client
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("could not ask GitHub for the release list")?
        .json()
        .await
        .context("GitHub sent a release list we could not read")?;
    let mut versions: Vec<Version> = releases.iter().filter_map(Release::version).collect();
    versions.sort_unstable_by(|a, b| b.cmp(a));
    Ok(versions)
}

fn asset_url(version: Version, file: &str) -> String {
    format!(
        "https://github.com/{REPO}/releases/download/{}/{file}",
        version.tag()
    )
}

/// Download the archive for `target` into `dest` and check it against the published checksum.
pub(super) async fn download(
    client: &reqwest::Client,
    version: Version,
    target: Target,
    dest: &Path,
) -> Result<()> {
    let url = asset_url(version, target.asset());
    tracing::debug!(%url, dest = %dest.display(), "downloading release archive");
    let response = client.get(&url).send().await.context("download failed")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("release {} has no binary for this platform", version.tag());
    }
    let response = response.error_for_status().context("download failed")?;

    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("could not write {}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("download was cut off")?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .context("could not write the download")?;
    }
    file.flush().await.context("could not write the download")?;

    let actual = format!("{:x}", hasher.finalize());
    let expected = published_checksum(client, version, target).await?;
    if actual != expected {
        bail!("checksum mismatch: expected {expected}, got {actual}. Nothing was installed.");
    }
    tracing::debug!(%actual, "checksum matches");
    Ok(())
}

async fn published_checksum(
    client: &reqwest::Client,
    version: Version,
    target: Target,
) -> Result<String> {
    let url = asset_url(version, &format!("{}.sha256", target.asset()));
    let text = client
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("could not fetch the checksum, so the download cannot be trusted")?
        .text()
        .await
        .context("could not read the checksum")?;
    text.split_whitespace()
        .next()
        .map(str::to_ascii_lowercase)
        .context("the checksum file is empty")
}
