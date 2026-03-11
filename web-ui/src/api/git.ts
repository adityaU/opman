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

// ── API functions ─────────────────────────────────────

export async function fetchGitStatus(): Promise<GitStatusResponse> {
  return apiFetch<GitStatusResponse>("/git/status");
}

export async function fetchGitDiff(file?: string, staged?: boolean): Promise<GitDiffResponse> {
  const params = new URLSearchParams();
  if (file) params.set("file", file);
  if (staged) params.set("staged", "true");
  const qs = params.toString();
  return apiFetch<GitDiffResponse>(`/git/diff${qs ? `?${qs}` : ""}`);
}

export async function fetchGitLog(limit?: number): Promise<GitLogResponse> {
  const qs = limit ? `?limit=${limit}` : "";
  return apiFetch<GitLogResponse>(`/git/log${qs}`);
}

export async function gitStage(files: string[] = []): Promise<void> {
  return apiPost("/git/stage", { files });
}

export async function gitUnstage(files: string[] = []): Promise<void> {
  return apiPost("/git/unstage", { files });
}

export async function gitCommit(message: string): Promise<GitCommitResponse> {
  return apiPost<GitCommitResponse>("/git/commit", { message });
}

export async function gitDiscard(files: string[]): Promise<void> {
  return apiPost("/git/discard", { files });
}

export async function fetchGitShow(hash: string): Promise<GitShowResponse> {
  return apiFetch<GitShowResponse>(
    `/git/show?hash=${encodeURIComponent(hash)}`
  );
}

export async function fetchGitBranches(): Promise<GitBranchesResponse> {
  return apiFetch<GitBranchesResponse>("/git/branches");
}

export async function gitCheckout(branch: string): Promise<GitCheckoutResponse> {
  return apiPost("/git/checkout", { branch });
}

export async function fetchGitRangeDiff(
  base?: string,
  limit?: number
): Promise<GitRangeDiffResponse> {
  const params = new URLSearchParams();
  if (base) params.set("base", base);
  if (limit != null) params.set("limit", String(limit));
  const qs = params.toString();
  return apiFetch<GitRangeDiffResponse>(`/git/range-diff${qs ? `?${qs}` : ""}`);
}

export async function fetchGitContextSummary(): Promise<GitContextSummaryResponse> {
  return apiFetch<GitContextSummaryResponse>("/git/context-summary");
}
