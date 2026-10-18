//! `bash`: run a command in the user's shell with bounded output and a hard deadline.

use super::capture::{Captured, HeadTail};
use super::limits::{ByteLimit, Timeout};
use super::output::{ToolError, ToolOutput, ToolResult};
use super::process::{shell_command, GroupKill};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};
use std::process::ExitStatus;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinSet;

const DEFAULT_TIMEOUT: Timeout = Timeout::from_secs(120);
const MAX_TIMEOUT: Timeout = Timeout::from_secs(600);
/// Per stream. About 30 KB, so one noisy command cannot fill the model's context.
const STREAM_LIMIT: ByteLimit = ByteLimit::new(30_000);
/// After the shell exits, how long to wait for a background child that still holds a pipe.
const PIPE_GRACE: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
pub struct BashArgs {
    command: String,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
    /// Only for the user's activity log; it never changes what runs.
    #[allow(dead_code)]
    description: Option<String>,
}

#[derive(Serialize)]
struct BashData {
    exit_code: Option<i32>,
    signal: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
    stdout_omitted_bytes: u64,
    stderr_omitted_bytes: u64,
    duration_ms: u64,
}

enum Stream {
    Out,
    Err,
}

enum Ended {
    Exited(ExitStatus),
    TimedOut,
}

pub async fn run(workspace: &Workspace, args: BashArgs) -> ToolResult {
    if args.command.trim().is_empty() {
        return Err(ToolError::new("command is empty"));
    }
    let cwd = match args.cwd.as_deref() {
        Some(raw) => workspace.resolve(raw)?,
        None => workspace.root().to_path_buf(),
    };
    if !tokio::fs::metadata(&cwd)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        return Err(ToolError::new(format!(
            "cwd {} is not a directory",
            workspace.show(&cwd)
        )));
    }
    let timeout = Timeout::requested_ms(args.timeout_ms, DEFAULT_TIMEOUT, MAX_TIMEOUT);

    let started = Instant::now();
    let mut command = shell_command(&args.command);
    command.current_dir(&cwd);
    let mut child = command
        .spawn()
        .map_err(|e| ToolError::new(format!("cannot start the shell: {e}")))?;
    let mut group = GroupKill::new(child.id());
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(ToolError::new("cannot read the command's output"));
    };
    // Dropping the set aborts the readers, so a cancelled call leaves no tasks behind.
    let mut readers = JoinSet::new();
    readers.spawn(drain(Stream::Out, stdout));
    readers.spawn(drain(Stream::Err, stderr));

    let ended = tokio::select! {
        status = child.wait() => match status {
            Ok(status) => Ended::Exited(status),
            Err(e) => return Err(ToolError::new(format!("cannot wait for the command: {e}"))),
        },
        () = tokio::time::sleep(timeout.get()) => Ended::TimedOut,
    };
    match &ended {
        Ended::Exited(_) => group.disarm(),
        Ended::TimedOut => {
            group.kill();
            let _ = child.kill().await;
        }
    }
    let (out, err) = collect(&mut readers).await;
    finish(ended, out, err, started.elapsed(), timeout)
}

async fn collect(readers: &mut JoinSet<(Stream, HeadTail)>) -> (Captured, Captured) {
    let (mut out, mut err) = (None, None);
    let gather = async {
        while let Some(Ok((stream, tail))) = readers.join_next().await {
            match stream {
                Stream::Out => out = Some(tail.finish()),
                Stream::Err => err = Some(tail.finish()),
            }
        }
    };
    let _ = tokio::time::timeout(PIPE_GRACE, gather).await;
    let empty = || Captured {
        text: String::new(),
        omitted: 0,
    };
    (out.unwrap_or_else(empty), err.unwrap_or_else(empty))
}

async fn drain<R: AsyncRead + Unpin>(stream: Stream, mut reader: R) -> (Stream, HeadTail) {
    let mut kept = HeadTail::new(STREAM_LIMIT);
    let mut buf = vec![0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => kept.push(&buf[..n]),
        }
    }
    (stream, kept)
}

fn finish(
    ended: Ended,
    out: Captured,
    err: Captured,
    elapsed: Duration,
    timeout: Timeout,
) -> ToolResult {
    let (exit_code, signal, timed_out) = match &ended {
        Ended::Exited(status) => (status.code(), signal_of(status), false),
        Ended::TimedOut => (None, None, true),
    };
    let mut text = out.text.clone();
    if !err.text.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("[stderr]\n");
        text.push_str(&err.text);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    match (&ended, exit_code, signal) {
        (Ended::TimedOut, _, _) => text.push_str(&format!(
            "[timed out after {} ms; the command and its child processes were killed]",
            timeout.get().as_millis()
        )),
        (_, Some(code), _) => text.push_str(&format!("[exit code {code}]")),
        (_, None, Some(sig)) => text.push_str(&format!("[killed by signal {sig}]")),
        _ => text.push_str("[exited without a code]"),
    }
    let data = BashData {
        exit_code,
        signal,
        timed_out,
        stdout: out.text,
        stderr: err.text,
        stdout_omitted_bytes: out.omitted,
        stderr_omitted_bytes: err.omitted,
        duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
    };
    if timed_out {
        return ToolOutput::failed(text, &data);
    }
    ToolOutput::ok(text, &data)
}

#[cfg(unix)]
fn signal_of(status: &ExitStatus) -> Option<i32> {
    std::os::unix::process::ExitStatusExt::signal(status)
}

#[cfg(not(unix))]
fn signal_of(_status: &ExitStatus) -> Option<i32> {
    None
}
