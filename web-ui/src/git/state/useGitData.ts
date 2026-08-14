/**
 * Everything the panel reads about one repository, refreshed together.
 *
 * A git mutation rarely changes only one thing — a checkout moves the branch,
 * the file list, the ahead/behind counts and possibly the worktree table at
 * once. Refreshing them as a set is what keeps the panel from showing a new
 * branch name above the old branch's files.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import * as api from "../api";
import type {
  GitBranches,
  GitCommit,
  GitOperation,
  GitStashEntry,
  GitStatus,
  GitSyncStatus,
  GitWorktree,
} from "../types";

export interface GitData {
  status: GitStatus | null;
  branches: GitBranches | null;
  sync: GitSyncStatus | null;
  worktrees: GitWorktree[];
  stashes: GitStashEntry[];
  operation: GitOperation | null;
  log: GitCommit[];
  loading: boolean;
  /** Set when the repository itself could not be read. */
  error: string | null;
  refresh: () => Promise<void>;
  /** Re-read the log alone, for "load more" and history-only changes. */
  refreshLog: (limit: number) => Promise<void>;
}

const LOG_PAGE = 100;

export function useGitData(scope: string): GitData {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [branches, setBranches] = useState<GitBranches | null>(null);
  const [sync, setSync] = useState<GitSyncStatus | null>(null);
  const [worktrees, setWorktrees] = useState<GitWorktree[]>([]);
  const [stashes, setStashes] = useState<GitStashEntry[]>([]);
  const [operation, setOperation] = useState<GitOperation | null>(null);
  const [log, setLog] = useState<GitCommit[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // A refresh triggered by a mutation can land after the user has already
  // switched repository; the token discards those instead of painting one
  // repository's data under another's name.
  const token = useRef(0);

  const refreshLog = useCallback(
    async (limit: number) => {
      const mine = token.current;
      const response = await api.fetchLog(scope, limit);
      if (mine === token.current) setLog(response.commits);
    },
    [scope],
  );

  const refresh = useCallback(async () => {
    const mine = ++token.current;
    try {
      const [statusResult, branchesResult, syncResult, worktreeResult, stashResult, operationResult, logResult] =
        await Promise.all([
          api.fetchStatus(scope),
          api.fetchBranches(scope),
          api.fetchSyncStatus(scope),
          api.fetchWorktrees(scope),
          api.stash(scope, "list"),
          api.fetchOperation(scope),
          api.fetchLog(scope, LOG_PAGE),
        ]);
      if (mine !== token.current) return;

      setStatus(statusResult);
      setBranches(branchesResult);
      setSync(syncResult);
      setWorktrees(worktreeResult.worktrees);
      setStashes(stashResult.entries);
      setOperation(operationResult.kind || operationResult.conflicted.length ? operationResult : null);
      setLog(logResult.commits);
      setError(null);
    } catch (cause) {
      if (mine !== token.current) return;
      const message = cause instanceof Error ? cause.message : String(cause);
      console.error("git read failed", cause);
      setError(message);
    } finally {
      if (mine === token.current) setLoading(false);
    }
  }, [scope]);

  useEffect(() => {
    setLoading(true);
    void refresh();
  }, [refresh]);

  return {
    status,
    branches,
    sync,
    worktrees,
    stashes,
    operation,
    log,
    loading,
    error,
    refresh,
    refreshLog,
  };
}
