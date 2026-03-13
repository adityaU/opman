import { apiFetch, apiPost } from "./client";

// ── Types ─────────────────────────────────────────────

export interface GitFileEntry {
  path: string;
  status: string;
}

export interface GitStatusResponse {
  branch: string;
  staged: GitFileEntry[];
  unstaged: GitFileEntry[];
  untracked: GitFileEntry[];
}

export interface GitDiffResponse {
  diff: string;
}

export interface GitLogEntry {
  hash: string;
  short_hash: string;
  author: string;
  date: string;
  message: string;
}

export interface GitLogResponse {
  commits: GitLogEntry[];
}

export interface GitCommitResponse {
  hash: string;
  message: string;
}

export interface GitShowFile {
  path: string;
  status: string;
}

export interface GitShowResponse {
  hash: string;
  author: string;
  date: string;
  message: string;
  diff: string;
  files: GitShowFile[];
}

export interface GitBranchesResponse {
  current: string;
  local: string[];
  remote: string[];
}

export interface GitCheckoutResponse {
  branch: string;
  success: boolean;
  message?: string;
}

export interface GitRangeDiffResponse {
  branch: string;
  base: string;
  commits: GitLogEntry[];
  diff: string;
  files_changed: number;
}

export interface GitContextSummaryResponse {
  branch: string;
  recent_commits: GitLogEntry[];
  staged_count: number;
  unstaged_count: number;
  untracked_count: number;
  summary: string;
}

// ── Multi-repo types ──────────────────────────────────

export interface GitRepoEntry {
  path: string;
  name: string;
  branch: string;
  staged_count: number;
  unstaged_count: number;
  untracked_count: number;
}

export interface GitReposResponse {
  repos: GitRepoEntry[];
}

// ── Helpers ───────────────────────────────────────────

/** Append optional `repo` param to a URLSearchParams */
function addRepoParam(params: URLSearchParams, repo?: string) {
  if (repo) params.set("repo", repo);
}

// ── API functions ─────────────────────────────────────

export async function fetchGitRepos(): Promise<GitReposResponse> {
  return apiFetch<GitReposResponse>("/git/repos");
}

export async function fetchGitStatus(repo?: string): Promise<GitStatusResponse> {
  const params = new URLSearchParams();
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitStatusResponse>(`/git/status${qs ? `?${qs}` : ""}`);
}

export async function fetchGitDiff(file?: string, staged?: boolean, repo?: string): Promise<GitDiffResponse> {
  const params = new URLSearchParams();
  if (file) params.set("file", file);
  if (staged) params.set("staged", "true");
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitDiffResponse>(`/git/diff${qs ? `?${qs}` : ""}`);
}

export async function fetchGitLog(limit?: number, repo?: string): Promise<GitLogResponse> {
  const params = new URLSearchParams();
  if (limit != null) params.set("limit", String(limit));
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitLogResponse>(`/git/log${qs ? `?${qs}` : ""}`);
}

export async function gitStage(files: string[] = [], repo?: string): Promise<void> {
  return apiPost("/git/stage", { files, repo });
}

export async function gitUnstage(files: string[] = [], repo?: string): Promise<void> {
  return apiPost("/git/unstage", { files, repo });
}

export async function gitCommit(message: string, repo?: string): Promise<GitCommitResponse> {
  return apiPost<GitCommitResponse>("/git/commit", { message, repo });
}

export async function gitDiscard(files: string[], repo?: string): Promise<void> {
  return apiPost("/git/discard", { files, repo });
}

export async function fetchGitShow(hash: string, repo?: string): Promise<GitShowResponse> {
  const params = new URLSearchParams();
  params.set("hash", hash);
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitShowResponse>(`/git/show?${qs}`);
}

export async function fetchGitBranches(repo?: string): Promise<GitBranchesResponse> {
  const params = new URLSearchParams();
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitBranchesResponse>(`/git/branches${qs ? `?${qs}` : ""}`);
}

export async function gitCheckout(branch: string, repo?: string): Promise<GitCheckoutResponse> {
  return apiPost("/git/checkout", { branch, repo });
}

export async function fetchGitRangeDiff(
  base?: string,
  limit?: number,
  repo?: string,
): Promise<GitRangeDiffResponse> {
  const params = new URLSearchParams();
  if (base) params.set("base", base);
  if (limit != null) params.set("limit", String(limit));
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitRangeDiffResponse>(`/git/range-diff${qs ? `?${qs}` : ""}`);
}

export async function fetchGitContextSummary(repo?: string): Promise<GitContextSummaryResponse> {
  const params = new URLSearchParams();
  addRepoParam(params, repo);
  const qs = params.toString();
  return apiFetch<GitContextSummaryResponse>(`/git/context-summary${qs ? `?${qs}` : ""}`);
}
