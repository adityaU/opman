/**
 * Repository discovery and scope selection.
 *
 * The previous panel defined a repo-refresh function, returned it, and never
 * called it — so the list stayed empty, the switcher's `length > 1` guard was
 * never true, and a monorepo was permanently pinned to its root. Here the
 * fetch runs from an effect keyed on the project, so discovery is not
 * something a caller can forget to trigger.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import { fetchRepos } from "../api";
import type { GitRepoEntry } from "../types";

/** The project root, which every repo path is relative to. */
export const ROOT_SCOPE = ".";

export interface GitReposState {
  repos: GitRepoEntry[];
  /** Currently scoped repository path, relative to the project root. */
  scope: string;
  setScope: (path: string) => void;
  /** The entry matching `scope`, when discovery has found it. */
  active: GitRepoEntry | null;
  loading: boolean;
  refresh: () => Promise<void>;
}

export function useGitRepos(projectPath: string): GitReposState {
  const [repos, setRepos] = useState<GitRepoEntry[]>([]);
  const [scope, setScope] = useState(ROOT_SCOPE);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const response = await fetchRepos();
      setRepos(response.repos);
    } catch (error) {
      // Discovery is an enhancement: the root repository still works without
      // it, so a failure here must not take the whole panel down.
      console.error("git repo discovery failed", error);
      setRepos([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // A different project is a different repository set, and the old scope
    // does not name anything in it.
    setScope(ROOT_SCOPE);
    setLoading(true);
    void refresh();
  }, [projectPath, refresh]);

  const active = useMemo(
    () => repos.find((repo) => repo.path === scope) ?? null,
    [repos, scope],
  );

  return { repos, scope, setScope, active, loading, refresh };
}
