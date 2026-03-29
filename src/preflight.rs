//! Pre-flight checks run at startup (non-blocking).
//!
//! Currently ensures required Docker Compose stacks are running.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;
use tracing::{info, warn};

/// A Docker Compose stack that must be running.
struct RequiredStack {
    compose_file: &'static str,
    containers: &'static [&'static str],
}

/// All stacks that opman expects to be running.
const REQUIRED_STACKS: &[RequiredStack] = &[RequiredStack {
    compose_file: "/home/ubuntu/opencode-tools/docker/searxng/docker-compose.yml",
    containers: &["searxng", "searxng-redis"],
}];

/// Spawn a background task that ensures all required Docker containers are up.
///
/// This is fire-and-forget: failures are logged but never block startup.
pub(crate) fn spawn_container_checks() {
    tokio::spawn(async {
        for stack in REQUIRED_STACKS {
            if let Err(e) = ensure_stack_running(stack).await {
                warn!(
                    "preflight: failed to ensure stack {} — {}",
                    stack.compose_file, e
                );
            }
        }
    });
}

/// Check whether every container in the stack is running; start the stack if not.
async fn ensure_stack_running(stack: &RequiredStack) -> anyhow::Result<()> {
    if !Path::new(stack.compose_file).exists() {
        warn!(
            "preflight: compose file missing, skipping: {}",
            stack.compose_file
        );
        return Ok(());
    }

    if !docker_available().await {
        warn!("preflight: docker not available, skipping container checks");
        return Ok(());
    }

    let all_running = containers_running(stack.containers).await?;
    if all_running {
        info!("preflight: all containers running for {}", stack.compose_file);
        return Ok(());
    }

    info!(
        "preflight: starting stack {} (some containers not running)",
        stack.compose_file
    );
    start_stack(stack.compose_file).await
}

/// Returns true if docker is on PATH and the daemon is reachable.
async fn docker_available() -> bool {
    Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Returns true only if *every* named container is in "running" state.
async fn containers_running(names: &[&str]) -> anyhow::Result<bool> {
    for name in names {
        let output = Command::new("docker")
            .args(["inspect", "-f", "{{.State.Running}}", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim() != "true" {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Run `docker compose up -d` for the given compose file.
async fn start_stack(compose_file: &str) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-f", compose_file, "up", "-d"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;

    if status.success() {
        info!("preflight: stack started successfully: {}", compose_file);
        Ok(())
    } else {
        anyhow::bail!(
            "docker compose up -d exited with code {:?}",
            status.code()
        )
    }
}
