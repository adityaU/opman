/**
 * The single way the panel runs a git mutation.
 *
 * The original branch-switch defect was not in the request — it was that a
 * refusal arrived as a 200 body and drained into an optional callback nobody
 * passed. Here a refusal has nowhere to go but the returned `result`, which
 * the panel renders, and every success refreshes before the caller is told it
 * finished. There is no path that reports "done" while the view still shows
 * the old branch.
 */

import { useCallback, useRef, useState } from "react";

import type { GitAction } from "../types";

export interface GitActionState {
  /** Label of the operation in flight, or null when idle. */
  pending: string | null;
  /** The last refusal. Cleared on the next attempt and by `dismiss`. */
  result: GitAction | null;
}

export interface GitActionRunner extends GitActionState {
  /**
   * Run a mutation, refresh, then resolve with the outcome.
   *
   * Resolves `null` when the request itself threw, having already recorded a
   * displayable result — callers branch on `ok`, never on a rejection.
   */
  run: (label: string, operation: () => Promise<GitAction>) => Promise<GitAction | null>;
  dismiss: () => void;
}

/**
 * @param refresh Re-reads everything the mutation could have changed. Awaited
 * before `run` resolves, so a caller that closes a dialog on success closes it
 * against fresh data.
 */
export function useGitAction(refresh: () => Promise<void>): GitActionRunner {
  const [state, setState] = useState<GitActionState>({ pending: null, result: null });

  // Refresh identity changes on every render of the owner; a ref keeps `run`
  // stable so it can sit in dependency arrays without re-subscribing anything.
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  const run = useCallback(async (label: string, operation: () => Promise<GitAction>) => {
    setState({ pending: label, result: null });
    try {
      const action = await operation();
      // Refresh even on refusal: a partial merge or a conflicted rebase
      // changes the repository whether or not the command reported success.
      await refreshRef.current();
      setState({ pending: null, result: action.ok ? null : action });
      return action;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error(`git ${label} failed`, error);
      setState({
        pending: null,
        result: { ok: false, failure: "failed", message },
      });
      return null;
    }
  }, []);

  const dismiss = useCallback(() => {
    setState((current) => (current.result ? { ...current, result: null } : current));
  }, []);

  return { ...state, run, dismiss };
}
