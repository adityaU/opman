//! A `bash` command that outlived the foreground limit: its output goes to files and a
//! supervisor task keeps watching it, so the call can return and the agent can check back.

use super::process::{signal_of, GroupKill};
use serde::Serialize;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::task::JoinSet;

/// After the shell exits, how long to wait for a child that still holds a pipe.
const PIPE_GRACE: Duration = Duration::from_millis(500);
/// A log stops growing here, so a noisy job cannot fill the disk.
const LOG_CAP: u64 = 64 * 1024 * 1024;

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

/// Where a job's output and result live. Removed on drop unless the job went to the
/// background, so quick commands leave nothing behind.
pub struct JobFiles {
    id: String,
    out: PathBuf,
    err: PathBuf,
    exit: PathBuf,
    keep: bool,
}

/// One output file. Writes stop quietly on the first error or at the size cap.
pub struct Log {
    file: File,
    written: u64,
}

#[derive(Serialize)]
pub struct JobInfo {
    id: String,
    pid: Option<u32>,
    stdout_file: String,
    stderr_file: String,
    exit_file: String,
}

enum Verdict {
    Exited(io::Result<ExitStatus>),
    TimedOut,
}

impl JobFiles {
    /// Two empty log files in the temp folder. `None` when they cannot be made: the command
    /// then still runs, it just cannot be moved to the background.
    pub async fn create() -> Option<(Self, Log, Log)> {
        let dir = std::env::temp_dir().join("opman-bash-jobs");
        tokio::fs::create_dir_all(&dir).await.ok()?;
        let id = format!(
            "{}-{}",
            std::process::id(),
            NEXT_JOB.fetch_add(1, Ordering::Relaxed)
        );
        let files = Self {
            out: dir.join(format!("{id}.out")),
            err: dir.join(format!("{id}.err")),
            exit: dir.join(format!("{id}.exit")),
            id,
            keep: false,
        };
        let out = Log::new(File::create(&files.out).await.ok()?);
        let err = Log::new(File::create(&files.err).await.ok()?);
        Some((files, out, err))
    }
}

impl Drop for JobFiles {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        let _ = std::fs::remove_file(&self.out);
        let _ = std::fs::remove_file(&self.err);
    }
}

impl Log {
    fn new(file: File) -> Self {
        Self { file, written: 0 }
    }

    /// `false` once the log is full or broken, so the caller can stop trying.
    pub async fn write(&mut self, chunk: &[u8]) -> bool {
        if self.written >= LOG_CAP {
            return false;
        }
        if self.file.write_all(chunk).await.is_err() {
            return false;
        }
        self.written += chunk.len() as u64;
        true
    }
}

/// Hand the running command to a supervisor task and say how to follow it.
pub fn detach<T: Send + 'static>(
    child: Child,
    group: GroupKill,
    readers: JoinSet<T>,
    mut files: JobFiles,
    kill_after: Option<Duration>,
) -> JobInfo {
    files.keep = true;
    let info = JobInfo {
        id: files.id.clone(),
        pid: child.id(),
        stdout_file: files.out.display().to_string(),
        stderr_file: files.err.display().to_string(),
        exit_file: files.exit.display().to_string(),
    };
    tokio::spawn(supervise(child, group, readers, files.exit.clone(), kill_after));
    info
}

async fn supervise<T: Send + 'static>(
    mut child: Child,
    mut group: GroupKill,
    mut readers: JoinSet<T>,
    exit_file: PathBuf,
    kill_after: Option<Duration>,
) {
    let verdict = match kill_after {
        Some(left) => tokio::select! {
            status = child.wait() => Verdict::Exited(status),
            () = tokio::time::sleep(left) => Verdict::TimedOut,
        },
        None => Verdict::Exited(child.wait().await),
    };
    match &verdict {
        Verdict::Exited(_) => group.disarm(),
        Verdict::TimedOut => {
            group.kill();
            let _ = child.kill().await;
        }
    }
    let gather = async { while readers.join_next().await.is_some() {} };
    let _ = tokio::time::timeout(PIPE_GRACE, gather).await;
    let _ = tokio::fs::write(&exit_file, verdict.line()).await;
}

impl Verdict {
    fn line(&self) -> String {
        match self {
            Self::TimedOut => "timed out; the command and its child processes were killed\n".into(),
            Self::Exited(Err(e)) => format!("could not be waited on: {e}\n"),
            Self::Exited(Ok(status)) => match (status.code(), signal_of(status)) {
                (Some(code), _) => format!("exit code {code}\n"),
                (None, Some(sig)) => format!("killed by signal {sig}\n"),
                (None, None) => "exited without a code\n".into(),
            },
        }
    }
}

impl JobInfo {
    /// What the agent reads: where the output is, and how to follow the job.
    pub fn instructions(&self, after: Duration) -> String {
        let pid = self.pid.map_or_else(|| "unknown".to_string(), |p| p.to_string());
        let stop = self
            .pid
            .map_or_else(String::new, |p| format!("\n  - Stop it: kill -- -{p}"));
        format!(
            "[still running after {} s, so it moved to the background. Job {}, pid {pid}]\n\
             Output so far, and everything it prints from now on:\n\
             \x20 stdout: {}\n\
             \x20 stderr: {}\n\
             When it ends, its result is written to {} (for example \"exit code 0\").\n\
             How to follow it:\n\
             \x20 - Read the latest output: tail -n 40 {}\n\
             \x20 - Check if it is done: cat {} (the file does not exist while it runs){stop}\n\
             \x20 - Pause between checks with the `wait` tool (for example 60 seconds). Do not poll in a tight loop.\n\
             Tell the agent or user who called you how it is going every few minutes while it \
             runs: what it printed lately, and whether it is done. Do not stay silent until it ends.",
            after.as_secs(),
            self.id,
            self.stdout_file,
            self.stderr_file,
            self.exit_file,
            self.stdout_file,
            self.exit_file,
        )
    }
}
