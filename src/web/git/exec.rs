//! The single place this codebase spawns `git`.
//!
//! Every call is non-interactive and bounded: git can neither open a
//! credential prompt nor outlive its deadline, so a handler cannot hang the
//! request thread waiting on a password that will never be typed.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

use crate::web::error::{WebError, WebResult};

/// Ceiling for a purely local command. Generous enough for `log` over a large
/// history, short enough that a wedged hook surfaces as an error.
const LOCAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Ceiling for anything that touches a remote.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

/// Whether a command is allowed to reach the network, which picks its deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    Local,
    Network,
}

impl Reach {
    const fn timeout(self) -> Duration {
        match self {
            Self::Local => LOCAL_TIMEOUT,
            Self::Network => NETWORK_TIMEOUT,
        }
    }
}

/// Why git refused. Each variant is a state the UI can offer a recovery for,
/// which is the whole reason this is an enum and not a message string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitFailure {
    /// The remote wants credentials that headless git cannot ask for.
    AuthRequired,
    /// Local modifications block the operation; stash or commit first.
    DirtyTree,
    /// A merge, rebase or cherry-pick stopped on conflicts.
    Conflict,
    /// The named ref, remote or path does not exist.
    NotFound,
    /// The remote moved on; fetch and integrate before pushing.
    Rejected,
    /// Another git process holds the index lock.
    Locked,
    /// Git ran and refused for a reason with no specific recovery.
    Failed,
}

impl GitFailure {
    /// A one-line recovery, phrased as the next thing to do.
    pub const fn hint(self) -> &'static str {
        match self {
            Self::AuthRequired => {
                "This remote needs credentials, and opman runs headless so it cannot ask for them. \
                 Configure an SSH key or a git credential helper on the server, then try again."
            }
            Self::DirtyTree => "Commit or stash your local changes first, then retry.",
            Self::Conflict => "Resolve the conflicting files, then continue or abort.",
            Self::NotFound => "Check the name — fetch first if it only exists on a remote.",
            Self::Rejected => "The remote has commits you do not. Pull, then push again.",
            Self::Locked => {
                "Another git process is using this repository. Wait for it to finish, or remove \
                 .git/index.lock if nothing is running."
            }
            Self::Failed => "See the git output below.",
        }
    }

    /// Classify from git's own diagnostics.
    fn classify(text: &str) -> Self {
        let lower = text.to_ascii_lowercase();
        let has = |needle: &str| lower.contains(needle);

        if has("could not read username")
            || has("could not read password")
            || has("authentication failed")
            || has("permission denied (publickey")
            || has("terminal prompts disabled")
            || has("host key verification failed")
        {
            return Self::AuthRequired;
        }
        if has("would be overwritten")
            || has("local changes")
            || has("you have unstaged changes")
            || has("cannot pull with rebase")
        {
            return Self::DirtyTree;
        }
        if has("conflict") || has("fix conflicts") || has("unmerged") {
            return Self::Conflict;
        }
        if has("index.lock") || has("unable to create") && has(".lock") {
            return Self::Locked;
        }
        if has("[rejected]") || has("non-fast-forward") || has("fetch first") {
            return Self::Rejected;
        }
        if has("did not match any file")
            || has("unknown revision")
            || has("not a valid object name")
            || has("does not appear to be a git repository")
            || has("no such remote")
            || has("pathspec")
        {
            return Self::NotFound;
        }
        Self::Failed
    }
}

/// A git process that exited non-zero, already classified.
#[derive(Debug, Clone)]
pub struct GitRefusal {
    pub failure: GitFailure,
    /// Git's own words — stderr, or stdout when stderr was silent.
    pub detail: String,
}

/// A literal tab inside a *pretty-format* string — `git log`, `git show`,
/// `git stash list`.
///
/// The two format dialects differ and the mistake is silent: `for-each-ref`
/// reads `%09` as a hex escape, while pretty-format reads only `%x09` and
/// prints `%09` verbatim, collapsing every column into one field.
pub const TAB: &str = "%x09";

/// A git process that exited zero.
#[derive(Debug, Clone)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
}

impl GitOutput {
    /// Trimmed stdout, which is what nearly every read path wants.
    pub fn trimmed(&self) -> &str {
        self.stdout.trim()
    }

    /// Non-empty lines of stdout.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.stdout.lines().map(str::trim).filter(|l| !l.is_empty())
    }

    /// What a user should be shown after a successful mutation. Git reports
    /// progress on stderr, so that is the interesting half.
    pub fn summary(&self) -> &str {
        let stderr = self.stderr.trim();
        if stderr.is_empty() {
            self.stdout.trim()
        } else {
            stderr
        }
    }
}

/// Outcome of one git invocation: it ran and succeeded, or ran and refused.
///
/// A failure to *spawn* is not modelled here — that is a server fault and
/// leaves as [`WebError::Internal`].
pub type GitResult = Result<GitOutput, GitRefusal>;

/// Run `git` in `dir` and classify the result.
pub async fn run(dir: &Path, args: &[&str], reach: Reach) -> WebResult<GitResult> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    harden(&mut command);

    let output = match tokio::time::timeout(reach.timeout(), command.output()).await {
        Ok(result) => result.map_err(|e| WebError::Internal(format!("Failed to run git: {e}")))?,
        Err(_) => {
            return Err(WebError::Internal(format!(
                "git {} timed out after {}s",
                args.first().copied().unwrap_or("?"),
                reach.timeout().as_secs()
            )));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        return Ok(Ok(GitOutput { stdout, stderr }));
    }

    let detail = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    Ok(Err(GitRefusal {
        failure: GitFailure::classify(&detail),
        detail: detail.trim().to_string(),
    }))
}

/// Run a read-only command whose refusal is not actionable, flattening a
/// refusal into an empty result.
///
/// Listing branches in a repository with no commits is the motivating case:
/// git exits non-zero, and the honest answer is an empty list, not an error.
pub async fn run_lenient(dir: &Path, args: &[&str]) -> WebResult<GitOutput> {
    Ok(run(dir, args, Reach::Local).await?.unwrap_or(GitOutput {
        stdout: String::new(),
        stderr: String::new(),
    }))
}

/// Run a command whose refusal should become an HTTP error.
pub async fn run_strict(dir: &Path, args: &[&str], reach: Reach) -> WebResult<GitOutput> {
    run(dir, args, reach)
        .await?
        .map_err(|refusal| match refusal.failure {
            GitFailure::NotFound => WebError::BadRequest(refusal.detail),
            _ => WebError::Internal(refusal.detail),
        })
}

/// Strip git of every way to become interactive or reach a user's config.
fn harden(command: &mut Command) {
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_PAGER", "cat")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10",
        );
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod exec_tests;
