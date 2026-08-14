/**
 * Wire types for the git API.
 *
 * Mutations answer with `GitAction`. A refusal is a normal 200 response with
 * `ok: false` and a `failure` code — never an exception — so a caller that
 * forgets to check it renders a stale view rather than throwing, and the
 * `useGitAction` runner exists so no caller has to remember.
 */

export type GitFailure =
  | "auth_required"
  | "dirty_tree"
  | "conflict"
  | "not_found"
  | "rejected"
  | "locked"
  | "failed";

export interface GitAction {
  ok: boolean;
  failure?: GitFailure;
  hint?: string;
  message: string;
}

export interface GitFileEntry {
  path: string;
  status: string;
}

export interface GitStatus {
  branch: string;
  staged: GitFileEntry[];
  unstaged: GitFileEntry[];
  untracked: GitFileEntry[];
}

export interface GitBranch {
  name: string;
  current: boolean;
  remote: boolean;
  upstream?: string;
  ahead: number;
  behind: number;
  subject: string;
  date: string;
  /** Set when another worktree holds this branch, which blocks checkout here. */
  worktree?: string;
}

export interface GitBranches {
  current: string;
  detached: boolean;
  local: GitBranch[];
  remote: GitBranch[];
  remotes: string[];
}

export interface GitRemote {
  name: string;
  fetchUrl: string;
  pushUrl?: string;
}

export interface GitSyncStatus {
  branch: string;
  detached: boolean;
  upstream?: string;
  ahead: number;
  behind: number;
  remotes: GitRemote[];
  unborn: boolean;
}

export interface GitWorktree {
  path: string;
  relative?: string;
  branch?: string;
  head: string;
  main: boolean;
  current: boolean;
  locked: boolean;
  prunable?: string;
}

export type GitOperationKind = "merge" | "rebase" | "cherry_pick" | "revert" | "bisect";

export interface GitOperation {
  kind?: GitOperationKind;
  conflicted: string[];
  step?: number;
  total?: number;
  onto?: string;
}

export interface GitStashEntry {
  index: number;
  reference: string;
  message: string;
  age: string;
  hash: string;
}

export type GitStashAction = "push" | "pop" | "apply" | "drop" | "list";

export interface GitStashResult extends GitAction {
  entries: GitStashEntry[];
}

export interface GitCommit {
  hash: string;
  short_hash: string;
  author: string;
  date: string;
  message: string;
}

export interface GitCommitResult extends GitAction {
  hash?: string;
}

export interface GitShowFile {
  path: string;
  status: string;
}

export interface GitShow {
  hash: string;
  author: string;
  date: string;
  message: string;
  diff: string;
  files: GitShowFile[];
}

export interface GitTag {
  name: string;
  hash: string;
  subject: string;
  date: string;
}

export interface GitBlameLine {
  hash: string;
  author: string;
  date: string;
  summary: string;
  line: number;
  content: string;
}

export type GitResetMode = "soft" | "mixed" | "hard";

export interface GitRepoEntry {
  path: string;
  name: string;
  branch: string;
  staged_count: number;
  unstaged_count: number;
  untracked_count: number;
}

/** The panel's top-level sections, in nav order. */
export type GitSection = "changes" | "history" | "branches" | "worktrees" | "stashes";
