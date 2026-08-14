//! Tag listing/creation/deletion and line blame.
//!
//! Every user string reaching an argv is parsed into one of the validated
//! newtypes in [`crate::web::git::refname`] first, and every filename is passed
//! after a `--` separator so it can never be read as a revision.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::WebResult;
use super::super::git::exec::{Reach, run, run_lenient, run_strict};
use super::super::git::refname::{self, CommitHash, RefName, RepoPath};
use super::super::git::scope;
use super::super::types::{
    GitActionResponse, GitBlameLine, GitBlameQuery, GitBlameResponse, GitRepoScope,
    GitTagDeleteRequest, GitTagEntry, GitTagRequest, GitTagsResponse, ServerState,
};

/// Field separator inside the for-each-ref row. A tag subject may legally
/// contain a tab, so the two subject variants are split on a record separator
/// the row can otherwise never carry.
const SUBJECT_SEP: char = '\u{1e}';

const TAG_FORMAT: &str = concat!(
    "%(refname:short)%09%(objectname)%09%(creatordate:iso-strict)%09",
    "%(contents:subject)\u{1e}%(subject)"
);

/// Most lines of blame to return, so a generated megafile cannot exhaust memory.
const BLAME_LINE_CAP: usize = 20000;

/// GET /api/git/tags — every tag, newest creation date first.
pub async fn git_tags(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &query.repo).await?;
    let format = format!("--format={TAG_FORMAT}");
    let output = run_lenient(
        repo.path(),
        &["for-each-ref", "refs/tags", "--sort=-creatordate", &format],
    )
    .await?;

    let tags = output.lines().filter_map(parse_tag_row).collect();
    Ok(Json(GitTagsResponse { tags }))
}

/// One `for-each-ref` row, or `None` when it is malformed and should be skipped.
fn parse_tag_row(line: &str) -> Option<GitTagEntry> {
    let mut fields = line.splitn(4, '\t');
    let name = fields.next()?;
    let hash = fields.next()?;
    let date = fields.next()?;
    let subjects = fields.next().unwrap_or("");
    if name.is_empty() || hash.is_empty() {
        return None;
    }
    // A lightweight tag has no annotation, so `contents:subject` is empty and
    // the commit's own `subject` is the meaningful one.
    let (annotated, plain) = match subjects.split_once(SUBJECT_SEP) {
        Some(pair) => pair,
        None => (subjects, ""),
    };
    let subject = if annotated.is_empty() { plain } else { annotated };
    Some(GitTagEntry {
        name: name.to_string(),
        hash: hash.to_string(),
        subject: subject.trim().to_string(),
        date: date.to_string(),
    })
}

/// POST /api/git/tag — create a lightweight or annotated tag.
pub async fn git_tag_create(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitTagRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let name = RefName::parse(&req.name)?;
    // A target may be either a hash or a name; hex is the stricter shape, so
    // it is tried first and a failure falls back to the ref grammar.
    let target = match req.target.as_deref() {
        Some(raw) => Some(match CommitHash::parse(raw) {
            Ok(hash) => hash.as_str(),
            Err(_) => RefName::parse(raw)?.as_str(),
        }),
        None => None,
    };
    let message = req.message.as_deref().map(refname::message).transpose()?;

    let mut args = vec!["tag"];
    if let Some(text) = message.as_deref() {
        args.extend(["-a", "-m", text]);
    }
    args.push(name.as_str());
    if let Some(target) = target {
        args.push(target);
    }

    let result = run(repo.path(), &args, Reach::Local).await?;
    Ok(Json(GitActionResponse::from(result)))
}

/// POST /api/git/tag/delete — remove a local tag.
pub async fn git_tag_delete(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitTagDeleteRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let name = RefName::parse(&req.name)?;
    let result = run(repo.path(), &["tag", "-d", name.as_str()], Reach::Local).await?;
    Ok(Json(GitActionResponse::from(result)))
}

/// GET /api/git/blame — per-line authorship for one file.
pub async fn git_blame(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitBlameQuery>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &query.repo).await?;
    let file = RepoPath::parse(&query.file)?;
    let output = run_strict(
        repo.path(),
        &["blame", "--line-porcelain", "--", file.as_str()],
        Reach::Local,
    )
    .await?;
    Ok(Json(GitBlameResponse {
        lines: parse_blame(&output.stdout),
    }))
}

/// What a commit contributes to every line it owns, cached once per sha.
#[derive(Default, Clone)]
struct BlameCommit {
    author: String,
    date: String,
    summary: String,
}

/// Parse `git blame --line-porcelain`.
///
/// Git emits the full header block only for the first line of each run of
/// consecutive lines from the same commit; later lines in that block carry the
/// sha line alone. Metadata is therefore cached per sha and carried forward.
fn parse_blame(stdout: &str) -> Vec<GitBlameLine> {
    let mut commits: HashMap<&str, BlameCommit> = HashMap::new();
    let mut lines = Vec::new();
    let mut current: Option<(&str, u32)> = None;
    let mut pending = BlameCommit::default();
    let mut author_time: Option<i64> = None;
    let mut author_tz: Option<&str> = None;

    for raw in stdout.lines() {
        if let Some(content) = raw.strip_prefix('\t') {
            let Some((sha, line)) = current.take() else {
                continue;
            };
            let entry = commits.entry(sha).or_default();
            // Only a full header block carries these; a continuation line
            // leaves the cached values in place.
            if !pending.author.is_empty() {
                entry.author = std::mem::take(&mut pending.author);
            }
            if !pending.summary.is_empty() {
                entry.summary = std::mem::take(&mut pending.summary);
            }
            if let (Some(time), Some(tz)) = (author_time.take(), author_tz.take()) {
                entry.date = iso_date(time, tz);
            }
            lines.push(GitBlameLine {
                hash: sha.to_string(),
                author: entry.author.clone(),
                date: entry.date.clone(),
                summary: entry.summary.clone(),
                line,
                content: content.to_string(),
            });
            if lines.len() >= BLAME_LINE_CAP {
                break;
            }
            continue;
        }

        let (key, value) = raw.split_once(' ').unwrap_or((raw, ""));
        match key {
            "author" => pending.author = value.to_string(),
            "summary" => pending.summary = value.to_string(),
            "author-time" => author_time = value.parse().ok(),
            "author-tz" => author_tz = Some(value),
            _ if current.is_none() && is_sha(key) => {
                // Header: <sha> <origline> <finalline> [<numlines>]
                let final_line = value
                    .split_whitespace()
                    .nth(1)
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                current = Some((key, final_line));
            }
            _ => {}
        }
    }
    lines
}

fn is_sha(token: &str) -> bool {
    token.len() >= 7 && token.chars().all(|c| c.is_ascii_hexdigit())
}

/// Unix seconds plus a `+HHMM` offset as an ISO-8601 timestamp in that zone.
pub(crate) fn iso_date(seconds: i64, tz: &str) -> String {
    let Some(utc) = chrono::DateTime::from_timestamp(seconds, 0) else {
        return String::new();
    };
    match parse_tz(tz).and_then(chrono::FixedOffset::east_opt) {
        Some(offset) => utc.with_timezone(&offset).to_rfc3339(),
        None => utc.to_rfc3339(),
    }
}

/// `+0530` or `-0800` as seconds east of UTC.
pub(crate) fn parse_tz(tz: &str) -> Option<i32> {
    let (sign, digits) = match tz.as_bytes().first()? {
        b'+' => (1, &tz[1..]),
        b'-' => (-1, &tz[1..]),
        _ => (1, tz),
    };
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits.get(..2)?.parse().ok()?;
    let minutes: i32 = digits.get(2..)?.parse().ok()?;
    Some(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
#[path = "git_refs_tests.rs"]
pub(crate) mod git_refs_tests;

#[cfg(test)]
#[path = "git_blame_tests.rs"]
mod git_blame_tests;
