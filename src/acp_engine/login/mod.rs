//! `opman acp-login <agent>`: log an ACP agent in from a terminal.
//!
//! Settings opens a terminal pane and runs this. An agent's login can need a person at a
//! keyboard (a menu, a pasted code) or a browser on this machine, neither of which the web
//! page can give it. Two kinds of method are handled:
//!
//! - **Terminal methods** (`"type": "terminal"` in `authMethods`): the agent's own command is
//!   run with the terminal's stdio, as the ACP spec describes.
//! - **Agent methods**: opman sends `authenticate`, shows whatever the agent prints (a link),
//!   and if the browser is elsewhere, takes the pasted `http://127.0.0.1:...` address and
//!   requests it here, where the agent is listening.

mod input;
mod methods;
mod relay;

use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::mpsc;

use super::config::{self, AgentConfig};
use super::launch;
use input::Input;
use methods::{Choice, Kind, Method};
use relay::Line;

/// How long a login may wait for the user to finish in the browser.
const LOGIN_LIMIT: Duration = Duration::from_secs(15 * 60);
const INIT_LIMIT: Duration = Duration::from_secs(60);

const PASTE_HINT: &str = "\nIf your browser is on another computer, it cannot finish the login by itself.\nApprove the login there, then copy the address the browser fails to open\n(it starts with http://127.0.0.1), paste it here and press Enter.\n";

/// Entry point for the CLI.
pub(crate) async fn run(id: &str, method: Option<&str>) -> Result<()> {
    let config = config::load();
    let agent = config
        .agents
        .get(id)
        .ok_or_else(|| anyhow!("no ACP agent named `{id}`"))?;
    if agent.command.is_empty() {
        bail!("`{id}` has no launch command; set one in Settings first");
    }
    let dir = std::env::temp_dir().join(format!("opman-login-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot make {}", dir.display()))?;
    let mut out = std::io::stdout();
    let outcome = drive(agent, &dir.to_string_lossy(), method, Input::terminal(), &mut out).await;
    let _ = std::fs::remove_dir(&dir);
    outcome
}

async fn drive(
    agent: &AgentConfig,
    dir: &str,
    wanted: Option<&str>,
    input: Input,
    out: &mut impl Write,
) -> Result<()> {
    let mut child = launch::spawn(agent, dir)?;
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        bail!("the agent process has no stdin/stdout pipes");
    };
    let (lines, events) = mpsc::channel(64);
    tokio::spawn(forward(stdout, lines.clone()));
    tokio::spawn(forward(stderr, lines));
    let mut talk = Talk {
        stdin,
        events,
        input,
        out,
        hinted: false,
        // Off until `authenticate`: before then typed lines answer the method menu, and
        // reading them as a pasted redirect would throw the answer away.
        typing: false,
    };
    let init = json!({ "protocolVersion": 1, "clientCapabilities": {} });
    let hello = tokio::time::timeout(INIT_LIMIT, talk.call(1, "initialize", init))
        .await
        .map_err(|_| anyhow!("the agent did not answer `initialize` in time"))??;
    let methods = methods::parse(&hello);
    let method = pick(&methods, wanted, &mut talk).await?;
    let name = method.name.clone();
    match &method.kind {
        Kind::Terminal { args, env } => {
            drop(talk);
            let _ = child.kill().await;
            terminal_login(agent, dir, args, env).await?;
        }
        Kind::Agent => {
            talk.typing = true;
            let params = json!({ "methodId": method.id });
            tokio::time::timeout(LOGIN_LIMIT, talk.call(2, "authenticate", params))
                .await
                .map_err(|_| anyhow!("the login did not finish within 15 minutes"))??;
        }
    }
    let _ = child.kill().await;
    writeln!(std::io::stdout(), "\nLogged in with \"{name}\". Back in Settings the agent now shows as Connected.")?;
    Ok(())
}

async fn pick<'a, W: Write>(
    methods: &'a [Method],
    wanted: Option<&str>,
    talk: &mut Talk<'_, W>,
) -> Result<&'a Method> {
    match methods::choose(methods, wanted).map_err(|reason| anyhow!(reason))? {
        Choice::Use(method) => Ok(method),
        Choice::Ask => {
            let _ = writeln!(talk.out, "How do you want to log in?");
            for (number, method) in methods.iter().enumerate() {
                let _ = writeln!(talk.out, "  {}. {}", number + 1, method.name);
            }
            let _ = write!(talk.out, "Number [1]: ");
            let _ = talk.out.flush();
            let answer = talk.input.line().await.unwrap_or_default();
            methods::by_number(methods, &answer).ok_or_else(|| anyhow!("`{}` is not one of the numbers above", answer.trim()))
        }
    }
}

/// Run the agent's own command in this terminal and wait for it.
async fn terminal_login(
    agent: &AgentConfig,
    dir: &str,
    args: &[String],
    env: &[(String, String)],
) -> Result<()> {
    let mut command = launch::command(agent, dir);
    command
        .args(args)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command
        .status()
        .await
        .with_context(|| format!("cannot run `{}`", agent.command))?;
    if !status.success() {
        bail!("`{}` ended with {status}", agent.command);
    }
    Ok(())
}

/// Read an agent's output line by line and hand each to the driver.
async fn forward<R: AsyncRead + Unpin>(stream: R, lines: mpsc::Sender<Line>) {
    let mut reader = BufReader::new(stream).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if lines.send(relay::classify(&line)).await.is_err() {
            return;
        }
    }
}

/// The conversation with the agent: requests out, replies and chatter in, and the user's
/// pasted redirect carried across.
struct Talk<'a, W: Write> {
    stdin: ChildStdin,
    events: mpsc::Receiver<Line>,
    input: Input,
    out: &'a mut W,
    /// The paste instructions are shown once, the first time a local address appears.
    hinted: bool,
    /// False once input has ended, so the select does not spin on it.
    typing: bool,
}

impl<W: Write> Talk<'_, W> {
    async fn call(&mut self, id: u64, method: &str, params: Value) -> Result<Value> {
        let mut frame = serde_json::to_string(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }))?;
        frame.push('\n');
        self.stdin.write_all(frame.as_bytes()).await.context("cannot write to the agent")?;
        self.stdin.flush().await.context("cannot write to the agent")?;
        self.reply_to(id, method).await
    }

    async fn reply_to(&mut self, id: u64, method: &str) -> Result<Value> {
        loop {
            tokio::select! {
                event = self.events.recv() => match event {
                    None => bail!("the agent closed its output during `{method}`"),
                    Some(Line::Text(text)) => self.show(&text),
                    Some(Line::Frame(frame)) => {
                        if let Some(outcome) = answer(&frame, id, method) {
                            return outcome;
                        }
                    }
                },
                typed = self.input.line(), if self.typing => match typed {
                    Some(line) => self.relay(&line).await,
                    None => self.typing = false,
                },
            }
        }
    }

    fn show(&mut self, text: &str) {
        let _ = writeln!(self.out, "{text}");
        if !self.hinted && relay::mentions_loopback(text) {
            self.hinted = true;
            let _ = write!(self.out, "{PASTE_HINT}");
        }
        let _ = self.out.flush();
    }

    async fn relay(&mut self, line: &str) {
        let Some(url) = relay::redirect_target(line) else {
            return;
        };
        let note = match relay::deliver(url).await {
            Ok(()) => "Sent the redirect to the agent.".to_string(),
            Err(error) => format!("{error:#}"),
        };
        let _ = writeln!(self.out, "{note}");
    }
}

/// The reply to request `id`, if this frame is it. Requests and notifications are not.
fn answer(frame: &Value, id: u64, method: &str) -> Option<Result<Value>> {
    if frame.get("method").is_some() || frame.get("id").and_then(Value::as_u64) != Some(id) {
        return None;
    }
    if let Some(error) = frame.get("error") {
        let message = error.get("message").and_then(Value::as_str).unwrap_or("unknown error");
        return Some(Err(anyhow!("`{method}` failed: {message}")));
    }
    Some(Ok(frame.get("result").cloned().unwrap_or(Value::Null)))
}

#[cfg(test)]
#[path = "login_tests.rs"]
mod login_tests;
