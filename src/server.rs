use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{debug, info, warn};

use crate::app::Project;
use crate::cli::AgentBackend;

/// Holds the child process for the managed agent server.
/// Wrapped in Arc<Mutex<>> so it can be shared with the ctrlc handler.
pub type ServerHandle = Arc<Mutex<Option<Child>>>;

/// Find a free TCP port by binding to port 0 and reading the assigned port.
fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind to port 0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Spawn the agent server for the given backend and wait for it to be ready.
///
/// - `opencode`: runs `opencode serve --port <port>` with optional inline config
/// - `claude-code`: runs `claude serve --port <port>`
///
/// Returns `(base_url, child_handle)`.
pub fn spawn_agent_server(
    backend: AgentBackend,
    opencode_config: Option<&str>,
) -> Result<(String, ServerHandle)> {
    let port = find_free_port().context("Could not find a free port")?;
    let binary = backend.binary();
    info!(port, %binary, "Spawning agent server");

    // Run from a temp directory so the server never picks up a config file
    // from the manager's own CWD.
    let temp = std::env::temp_dir();
    let mut command = Command::new(binary);
    command
        .args(["serve", "--port", &port.to_string()])
        .current_dir(&temp)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(config) = opencode_config {
        command.env("OPENCODE_CONFIG_CONTENT", config);
    }
    let mut child = command
        .spawn()
        .with_context(|| {
            format!("Failed to spawn `{binary} serve`. Is {binary} installed and on PATH?")
        })?;

    let stdout = child
        .stdout
        .take()
        .context("Failed to capture agent server stdout")?;

    let reader = BufReader::new(stdout);
    let mut base_url: Option<String> = None;

    // Allow up to 20 seconds for the server to print its listening line.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);

    for line in reader.lines() {
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            bail!("Timed out waiting for `{binary} serve` to start (20s)");
        }
        match line {
            Ok(line) => {
                debug!(%binary, %line, "agent server stdout");
                // Both opencode and claude-code print a line containing "http://"
                if let Some(url_start) = line.find("http://") {
                    let url = line[url_start..].trim().to_string();
                    info!(%url, %binary, "Agent server is ready");
                    base_url = Some(url);
                    break;
                }
            }
            Err(e) => {
                warn!("Error reading agent server stdout: {}", e);
                break;
            }
        }
    }

    let url = base_url.unwrap_or_else(|| {
        warn!("Could not parse listening URL from `{binary} serve` output, using fallback");
        format!("http://127.0.0.1:{}", port)
    });

    let handle: ServerHandle = Arc::new(Mutex::new(Some(child)));
    Ok((url, handle))
}

/// Kill the managed agent server if it's still running.
pub fn kill_server(handle: &ServerHandle) {
    if let Ok(mut guard) = handle.lock() {
        if let Some(ref mut child) = *guard {
            info!("Shutting down managed agent server (pid={})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }
}

/// Shut down all PTY processes for every project.
pub fn shutdown_all_ptys(projects: &mut [Project]) {
    for project in projects.iter_mut() {
        // Kill opencode session PTYs
        for (_, pty) in project.ptys.iter_mut() {
            let _ = pty.kill();
        }
        project.ptys.clear();

        // Kill per-session resources (shell terminals + neovim instances)
        for resources in project.session_resources.values_mut() {
            for shell_pty in &mut resources.shell_ptys {
                let _ = shell_pty.kill();
            }
            if let Some(ref mut nvim) = resources.neovim_pty {
                let _ = nvim.kill();
            }
        }
        project.session_resources.clear();

        // Kill gitui PTY if running
        if let Some(ref mut gitui) = project.gitui_pty {
            let _ = gitui.kill();
        }
        project.gitui_pty = None;

        debug!(name = %project.name, "All PTYs killed during shutdown");
        project.active_session = None;
    }
    info!("All PTYs shut down");
}
