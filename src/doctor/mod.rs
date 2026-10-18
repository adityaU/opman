//! `opman doctor`: find what is wrong with an opman setup and say so plainly.
//!
//! Every check returns a [`Check`]. Independent groups run at the same time, each under its
//! own time limit, so one stuck agent cannot hold up the report.

mod acp;
mod environment;
mod mcp;
mod render;
mod report;
mod runners;
mod server;
mod tools;

use std::future::Future;
use std::io::IsTerminal;
use std::time::Duration;

use anyhow::Result;

use crate::acp_engine::config;
use report::{Report, Section};

const QUICK_LIMIT: Duration = Duration::from_secs(10);
const MCP_LIMIT: Duration = Duration::from_secs(30);
const ACP_LIMIT: Duration = Duration::from_secs(50);

/// Run a group of checks, or report that it did not finish in time.
async fn limited(
    name: &'static str,
    limit: Duration,
    work: impl Future<Output = Section>,
) -> Section {
    tokio::time::timeout(limit, work)
        .await
        .unwrap_or_else(|_| Section::timed_out(name, limit.as_secs()))
}

/// Gather every section. `only` limits the ACP checks to one agent.
async fn gather(only: Option<&str>) -> Report {
    let acp_config = config::load();
    let agents = acp::selected(&acp_config, only);
    let commands: Vec<&str> = agents
        .iter()
        .flatten()
        .map(|(_, agent)| agent.command.as_str())
        .collect();
    let path = std::env::var_os("PATH");

    let environment = async { Section::new("Environment", environment::checks().await) };
    let tools = async { Section::new("Tools on PATH", tools::checks(&commands, path.as_deref())) };
    let (environment, tools, acp, mcp, server, runners) = tokio::join!(
        limited("Environment", QUICK_LIMIT, environment),
        limited("Tools on PATH", QUICK_LIMIT, tools),
        limited("ACP agents", ACP_LIMIT, acp::section(agents)),
        limited("MCP servers", MCP_LIMIT, mcp::section()),
        limited("Running server", QUICK_LIMIT, server::section()),
        limited("Runner CLIs", QUICK_LIMIT, runners::section()),
    );
    Report::new(vec![environment, tools, acp, mcp, server, runners])
}

/// `opman doctor`. Prints the report and exits with 1 when any check failed.
pub(crate) async fn run(json: bool, agent: Option<String>) -> Result<()> {
    let report = gather(agent.as_deref()).await;
    if json {
        println!("{}", serde_json::to_string_pretty(&render::json(&report))?);
    } else {
        let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        print!("{}", render::human(&report, color));
    }
    if report.exit_code() != 0 {
        std::process::exit(report.exit_code());
    }
    Ok(())
}
