//! `bash`: run a command in the user's shell with bounded output and a hard deadline.

use super::bash_job::{self, JobFiles, JobInfo, Log};
use super::capture::{Captured, HeadTail};
use super::limits::{ByteLimit, RunLimit, Timeout};
use super::output::{ToolError, ToolOutput, ToolResult};
use super::process::{shell_command, signal_of, GroupKill};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};
use std::process::ExitStatus;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinSet;

/// A command still running after this long moves to the background instead of being killed.
const BACKGROUND_AFTER: Timeout = Timeout::from_secs(600);
/// The longest a background job may run when the caller asks for a timeout.
const HARD_MAX: Timeout = Timeout::from_secs(24 * 60 * 60);
/// Per stream. About 30 KB, so one noisy command cannot fill the model's context.
const STREAM_LIMIT: ByteLimit = ByteLimit::new(30_000);
/// After the shell exits, how long to wait for a background child that still holds a pipe.
const PIPE_GRACE: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
pub struct BashArgs {
    #[serde(alias = "cmd", alias = "command_line", alias = "CommandLine")]
    command: String,
    #[serde(alias = "workdir", alias = "working_dir", alias = "workingDir")]
    cwd: Option<String>,
    timeout_ms: Option<u64>,
    /// Only for the user's activity log; it never changes what runs.
    #[allow(dead_code)]
    #[serde(alias = "toolSummary")]
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
    background: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    job: Option<JobInfo>,
}

enum Stream {
    Out,
    Err,
}

enum Ended {
    Exited(ExitStatus),
    TimedOut,
    /// Still running at the foreground limit: handed to a background supervisor.
    Background,
}

pub async fn run(workspace: &Workspace, args: BashArgs) -> ToolResult {
    run_after(workspace, args, BACKGROUND_AFTER).await
}

/// [`run`] with the foreground limit chosen by the caller, so tests need not wait 10 minutes.
pub(super) async fn run_after(
    workspace: &Workspace,
    args: BashArgs,
    background_after: Timeout,
) -> ToolResult {
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
    let limit = RunLimit::requested_ms(args.timeout_ms, background_after, HARD_MAX);

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
    let (files, out_log, err_log) = match JobFiles::create().await {
        Some((files, out, err)) => (Some(files), Some(out), Some(err)),
        None => (None, None, None),
    };
    let mut readers = JoinSet::new();
    readers.spawn(drain(Stream::Out, stdout, out_log));
    readers.spawn(drain(Stream::Err, stderr, err_log));

    let ended = tokio::select! {
        status = child.wait() => match status {
            Ok(status) => Ended::Exited(status),
            Err(e) => return Err(ToolError::new(format!("cannot wait for the command: {e}"))),
        },
        () = tokio::time::sleep(limit.first_deadline()) => match (limit, files.is_some()) {
            (RunLimit::Background { .. }, true) => Ended::Background,
            _ => Ended::TimedOut,
        },
    };
    match (&ended, limit, files) {
        (Ended::Background, RunLimit::Background { after, kill_at }, Some(files)) => {
            let kill_after = kill_at.map(|total| total.get().saturating_sub(started.elapsed()));
            let job = bash_job::detach(child, group, readers, files, kill_after);
            return background_result(job, after.get(), started.elapsed());
        }
        (Ended::Exited(_), ..) => group.disarm(),
        _ => {
            group.kill();
            let _ = child.kill().await;
        }
    }
    let (out, err) = collect(&mut readers).await;
    finish(ended, out, err, started.elapsed(), limit.first_deadline())
}

fn background_result(job: JobInfo, after: Duration, elapsed: Duration) -> ToolResult {
    let text = job.instructions(after);
    let data = BashData {
        exit_code: None,
        signal: None,
        timed_out: false,
        stdout: String::new(),
        stderr: String::new(),
        stdout_omitted_bytes: 0,
        stderr_omitted_bytes: 0,
        duration_ms: millis(elapsed),
        background: true,
        job: Some(job),
    };
    ToolOutput::ok(text, &data)
}

fn millis(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
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

/// Reads one stream to its end, keeping head and tail in memory and copying everything to
/// `log`, so the full output is on disk if the command moves to the background.
async fn drain<R: AsyncRead + Unpin>(
    stream: Stream,
    mut reader: R,
    mut log: Option<Log>,
) -> (Stream, HeadTail) {
    let mut kept = HeadTail::new(STREAM_LIMIT);
    let mut buf = vec![0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                kept.push(&buf[..n]);
                if let Some(open) = log.as_mut() {
                    if !open.write(&buf[..n]).await {
                        log = None;
                    }
                }
            }
        }
    }
    (stream, kept)
}

fn finish(
    ended: Ended,
    out: Captured,
    err: Captured,
    elapsed: Duration,
    limit: Duration,
) -> ToolResult {
    let (exit_code, signal, timed_out) = match &ended {
        Ended::Exited(status) => (status.code(), signal_of(status), false),
        Ended::TimedOut | Ended::Background => (None, None, true),
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
            limit.as_millis()
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
        duration_ms: millis(elapsed),
        background: false,
        job: None,
    };
    if timed_out {
        return ToolOutput::failed(text, &data);
    }
    ToolOutput::ok(text, &data)
}
