//! Write a file so a reader never sees half of it: write a sibling file, then rename.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use tokio::io::AsyncWriteExt;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub async fn atomic_write(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(".{name}.opman-{}-{id}.tmp", std::process::id()));
    let result = write_then_rename(&temp, target, bytes).await;
    if result.is_err() {
        let _ = fs::remove_file(&temp).await;
    }
    result
}

async fn write_then_rename(temp: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(temp).await?;
    file.write_all(bytes).await?;
    file.flush().await?;
    drop(file);
    // Keep the mode of the file being replaced, such as an executable bit.
    if let Ok(existing) = fs::metadata(target).await {
        fs::set_permissions(temp, existing.permissions()).await?;
    }
    fs::rename(temp, target).await
}
