//! File logging. Everything goes to `<config_dir>/opman/opman.log`; stderr stays free for the TUI.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

const DEFAULT_FILTER: &str = "info,notify=error,walkdir=error";
/// `debug` for opman, with the chatty network crates held at `info` so the file stays readable.
const DEBUG_FILTER: &str = "debug,notify=error,walkdir=error,hyper=info,h2=info,rustls=info,reqwest=info,tungstenite=info,tokio_tungstenite=info";

/// Where the log file lives.
pub(crate) fn log_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("could not determine the config directory")?
        .join("opman");
    std::fs::create_dir_all(&dir).context("could not create the log directory")?;
    Ok(dir.join("opman.log"))
}

/// Start logging to the log file. `--debug` beats `RUST_LOG`, so the flag always does what it says.
pub(crate) fn init(debug: bool) -> Result<PathBuf> {
    let path = log_path()?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("could not open {}", path.display()))?;
    let filter = if debug {
        EnvFilter::new(DEBUG_FILTER)
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER))
    };
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(file))
        .with_env_filter(filter)
        .init();
    Ok(path)
}
