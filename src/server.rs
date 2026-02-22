use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{debug, info, warn};

use crate::app::Project;

/// Holds the child process for the managed opencode server.
/// Wrapped in Arc<Mutex<>> so it can be shared with the ctrlc handler.
pub type ServerHandle = Arc<Mutex<Option<Child>>>;

/// Find a free TCP port by binding to port 0 and reading the assigned port.
fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind to port 0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Spawn `opencode serve --port <port>` and wait for the "listening on" line.
/// Returns (base_url, child_handle).
pub fn spawn_opencode_server() -> Result<(String, ServerHandle)> {
    let port = find_free_port().context("Could not find a free port")?;
    info!(port, "Spawning opencode serve");

    // Run opencode serve from a temp directory so it never picks up an
    // opencode.json that lives in the manager's own CWD.
    let temp = std::env::temp_dir();
    let mut child = Command::new("opencode")
        .args(["serve", "--port", &port.to_string()])
        .current_dir(&temp)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn `opencode serve`. Is opencode installed and on PATH?")?;

    // Read stdout line-by-line until we see the "listening on" message.
    let stdout = child
        .stdout
        .take()
        .context("Failed to capture opencode serve stdout")?;

    let reader = BufReader::new(stdout);
    let mut base_url: Option<String> = None;

    // Give it up to 15 seconds to print the listening line.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);

    for line in reader.lines() {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            bail!("Timed out waiting for opencode serve to start (15s)");
        }
        match line {
            Ok(line) => {
                // Expected: "opencode server listening on http://127.0.0.1:PORT"
                if let Some(url_start) = line.find("http://") {
                    let url = line[url_start..].trim().to_string();
                    info!(%url, "opencode serve is ready");
                    base_url = Some(url);
                    break;
                }
            }
            Err(e) => {
                warn!("Error reading opencode serve stdout: {}", e);
                break;
            }
        }
    }

    let url = base_url.unwrap_or_else(|| {
        // Fallback: assume the port we requested is being used
        warn!("Could not parse listening URL from opencode serve output, using fallback");
        format!("http://127.0.0.1:{}", port)
    });

    let handle: ServerHandle = Arc::new(Mutex::new(Some(child)));
    Ok((url, handle))
}

/// Kill the managed opencode server if it's still running.
pub fn kill_server(handle: &ServerHandle) {
    if let Ok(mut guard) = handle.lock() {
        if let Some(ref mut child) = *guard {
            info!("Shutting down managed opencode serve (pid={})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }
}

/// Shut down all PTY processes for every project.
pub fn shutdown_all_ptys(projects: &mut [Project]) {
    for project in projects.iter_mut() {
        for (_, pty) in project.ptys.iter_mut() {
            let _ = pty.kill();
        }
        debug!(name = %project.name, "PTYs killed during shutdown");
        project.ptys.clear();
        project.active_session = None;
    }
    info!("All PTYs shut down");
}
