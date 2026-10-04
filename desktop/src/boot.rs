//! Startup sequence: spawn opman, wait for its web server, then point the window at it.

use std::fmt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::sidecar::{Port, Sidecar, SidecarError};
use crate::splash::Splash;

const MAIN_WINDOW: &str = "main";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

enum BootError {
    Sidecar(SidecarError),
    Tauri(tauri::Error),
    Url(url::ParseError),
}

impl From<SidecarError> for BootError {
    fn from(err: SidecarError) -> Self {
        Self::Sidecar(err)
    }
}

impl From<tauri::Error> for BootError {
    fn from(err: tauri::Error) -> Self {
        Self::Tauri(err)
    }
}

impl From<url::ParseError> for BootError {
    fn from(err: url::ParseError) -> Self {
        Self::Url(err)
    }
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sidecar(err) => err.fmt(f),
            Self::Tauri(err) => write!(f, "Desktop shell error: {err}"),
            Self::Url(err) => write!(f, "Invalid opman URL: {err}"),
        }
    }
}

/// Runs on a background thread for the life of the app.
pub fn run(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        eprintln!("opman-desktop: window '{MAIN_WINDOW}' does not exist");
        return;
    };
    let splash = match Splash::capture(&window) {
        Ok(splash) => splash,
        Err(err) => {
            eprintln!("opman-desktop: cannot read splash URL: {err}");
            return;
        }
    };
    let log_path = match log_path(app) {
        Ok(path) => path,
        Err(err) => return splash.fail(&err.to_string()),
    };

    let err = match start(app, &window, &log_path) {
        Ok(()) => watch(app),
        Err(err) => err,
    };
    // The app is quitting; nothing to report.
    if matches!(err, BootError::Sidecar(SidecarError::Stopped)) {
        return;
    }
    splash.fail(&format!("{err}\n\nLog file: {}", log_path.display()));
}

fn log_path(app: &AppHandle) -> Result<PathBuf, BootError> {
    let dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&dir).map_err(SidecarError::Log)?;
    Ok(dir.join("opman.log"))
}

fn start(app: &AppHandle, window: &WebviewWindow, log_path: &Path) -> Result<(), BootError> {
    let sidecar = app.state::<Sidecar>();
    let port = Port::ephemeral().map_err(SidecarError::Port)?;
    sidecar.spawn(port, log_path)?;
    sidecar.wait_until_ready(port, STARTUP_TIMEOUT)?;
    window.navigate(port.url()?)?;
    Ok(())
}

/// Blocks until opman stops serving and returns why.
fn watch(app: &AppHandle) -> BootError {
    let sidecar = app.state::<Sidecar>();
    loop {
        thread::sleep(WATCH_INTERVAL);
        if let Err(err) = sidecar.check() {
            return err.into();
        }
    }
}
