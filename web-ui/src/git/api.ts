/**
 * Every git endpoint, in one place.
 *
 * `repo` is threaded through every call because the panel can be scoped to a
 * nested repository or a linked worktree; leaving it off silently targets the
 * project root, which is the wrong repository more often than not.
 */

import { apiFetch, apiPost } from "../api/client";
import type {
  GitAction,
  GitBlameLine,
  GitBranches,
  GitCommit,
  GitCommitResult,
  GitOperation,
  GitRepoEntry,
  GitResetMode,
  GitShow,
  GitStashAction,
  GitStashResult,
  GitStatus,
  GitSyncStatus,
  GitTag,
  GitWorktree,
} from "./types";

/** Build a query string, always carrying the repo scope. */
function query(repo: string, extra?: Record<string, string | number | undefined>): string {
  const params = new URLSearchParams();
  if (repo && repo !== ".") params.set("repo", repo);
  for (const [key, value] of Object.entries(extra ?? {})) {
    if (value !== undefined) params.set(key, String(value));
  }
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

// ── Reads ─────────────────────────────────────────────

export const fetchStatus = (repo: string) =>
  apiFetch<GitStatus>(`/git/status${query(repo)}`);

export const fetchLog = (repo: string, limit = 100) =>
  apiFetch<{ commits: GitCommit[] }>(`/git/log${query(repo, { limit })}`);

export const fetchBranches = (repo: string) =>
  apiFetch<GitBranches>(`/git/branches${query(repo)}`);

export const fetchSyncStatus = (repo: string) =>
  apiFetch<GitSyncStatus>(`/git/sync-status${query(repo)}`);

export const fetchWorktrees = (repo: string) =>
  apiFetch<{ worktrees: GitWorktree[] }>(`/git/worktrees${query(repo)}`);

export const fetchOperation = (repo: string) =>
  apiFetch<GitOperation>(`/git/operation${query(repo)}`);

export const fetchTags = (repo: string) =>
  apiFetch<{ tags: GitTag[] }>(`/git/tags${query(repo)}`);

export const fetchRepos = () => apiFetch<{ repos: GitRepoEntry[] }>("/git/repos");

export const fetchDiff = (repo: string, file?: string, staged = false) =>
  apiFetch<{ diff: string }>(`/git/diff${query(repo, { file, staged: String(staged) })}`);

export const fetchShow = (repo: string, hash: string) =>
  apiFetch<GitShow>(`/git/show${query(repo, { hash })}`);

export const fetchBlame = (repo: string, file: string) =>
  apiFetch<{ lines: GitBlameLine[] }>(`/git/blame${query(repo, { file })}`);

// ── Working tree ──────────────────────────────────────

export const stage = (repo: string, files: string[]) =>
  apiPost<void>("/git/stage", { files, repo });

export const unstage = (repo: string, files: string[]) =>
  apiPost<void>("/git/unstage", { files, repo });

export const discard = (repo: string, files: string[]) =>
  apiPost<void>("/git/discard", { files, repo });

export const commit = (
  repo: string,
  message: string,
  options: { amend?: boolean; stageAll?: boolean } = {},
) => apiPost<GitCommitResult>("/git/commit", { message, repo, ...options });

// ── Branches ──────────────────────────────────────────

export const checkout = (repo: string, branch: string, carryChanges = false) =>
  apiPost<GitAction>("/git/checkout", { branch, repo, carryChanges });

export const createBranch = (
  repo: string,
  name: string,
  options: { startPoint?: string; checkout?: boolean } = {},
) => apiPost<GitAction>("/git/branch/create", { name, repo, ...options });

export const deleteBranch = (
  repo: string,
  name: string,
  options: { force?: boolean; remote?: string } = {},
) => apiPost<GitAction>("/git/branch/delete", { name, repo, ...options });

export const renameBranch = (repo: string, from: string, to: string) =>
  apiPost<GitAction>("/git/branch/rename", { from, to, repo });

// ── Remotes ───────────────────────────────────────────

export const fetchRemote = (repo: string, options: { remote?: string; prune?: boolean } = {}) =>
  apiPost<GitAction>("/git/fetch", { repo, ...options });

export const pull = (repo: string) => apiPost<GitAction>("/git/pull", { repo });

export const push = (
  repo: string,
  options: { remote?: string; branch?: string; setUpstream?: boolean; force?: boolean } = {},
) => apiPost<GitAction>("/git/push", { repo, ...options });

// ── Worktrees ─────────────────────────────────────────

export const addWorktree = (
  repo: string,
  path: string,
  branch: string,
  options: { create?: boolean; startPoint?: string } = {},
) => apiPost<GitAction>("/git/worktree/add", { repo, path, branch, ...options });

export const removeWorktree = (repo: string, path: string, force = false) =>
  apiPost<GitAction>("/git/worktree/remove", { repo, path, force });

export const pruneWorktrees = (repo: string) => apiPost<GitAction>("/git/worktree/prune", { repo });

// ── Integration ───────────────────────────────────────

export const merge = (repo: string, branch: string, options: { noFf?: boolean } = {}) =>
  apiPost<GitAction>("/git/merge", { repo, branch, ...options });

export const rebase = (repo: string, onto: string) =>
  apiPost<GitAction>("/git/rebase", { repo, onto });

export const resolveOperation = (repo: string, action: "continue" | "abort" | "skip") =>
  apiPost<GitAction>("/git/operation", { repo, action });

export const reset = (repo: string, target: string, mode: GitResetMode) =>
  apiPost<GitAction>("/git/reset", { repo, target, mode });

export const revert = (repo: string, hash: string) =>
  apiPost<GitAction>("/git/revert", { repo, hash });

export const cherryPick = (repo: string, hash: string) =>
  apiPost<GitAction>("/git/cherry-pick", { repo, hash });

// ── Stashes ───────────────────────────────────────────

export const stash = (
  repo: string,
  action: GitStashAction,
  options: { message?: string; stashRef?: string } = {},
) => apiPost<GitStashResult>("/git/stash", { repo, action, ...options });

// ── Tags ──────────────────────────────────────────────

export const createTag = (
  repo: string,
  name: string,
  options: { message?: string; target?: string } = {},
) => apiPost<GitAction>("/git/tag", { repo, name, ...options });

export const deleteTag = (repo: string, name: string) =>
  apiPost<GitAction>("/git/tag/delete", { repo, name });
