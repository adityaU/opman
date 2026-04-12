import { useState, useCallback, useRef } from "react";
import { gitStage, gitUnstage, gitCommit, gitDiscard, gitCheckout } from "../../api";

/**
 * Git staging, commit, discard, and checkout actions.
 * All operations forward the selected repo path to the API.
 */
export function useGitActions(
  branch: string,
  setBranch: (b: string) => void,
  refreshStatus: () => Promise<void>,
  selectedRepo: string | undefined,
  onError?: (msg: string) => void,
) {
  const [commitMsg, setCommitMsg]     = useState("");
  const [committing, setCommitting]   = useState(false);
  const [checkingOut, setCheckingOut] = useState(false);

  // Keep a ref so callbacks always use the latest value
  const repoRef = useRef(selectedRepo);
  repoRef.current = selectedRepo;

  // ── Stage / unstage ───────────────────────────────────

  const handleStage = useCallback(async (files: string[]) => {
    try { await gitStage(files, repoRef.current); await refreshStatus(); }
    catch (err) { console.error("Failed to stage:", err); onError?.("Failed to stage file"); }
  }, [refreshStatus, onError]);

  const handleUnstage = useCallback(async (files: string[]) => {
    try { await gitUnstage(files, repoRef.current); await refreshStatus(); }
    catch (err) { console.error("Failed to unstage:", err); onError?.("Failed to unstage file"); }
  }, [refreshStatus, onError]);

  const handleStageAll = useCallback(async () => {
    try { await gitStage([], repoRef.current); await refreshStatus(); }
    catch (err) { console.error("Failed to stage all:", err); onError?.("Failed to stage all files"); }
  }, [refreshStatus, onError]);

  const handleUnstageAll = useCallback(async () => {
    try { await gitUnstage([], repoRef.current); await refreshStatus(); }
    catch (err) { console.error("Failed to unstage all:", err); onError?.("Failed to unstage all files"); }
  }, [refreshStatus, onError]);

  // ── Discard ───────────────────────────────────────────

  const handleDiscard = useCallback(async (files: string[]) => {
    if (!window.confirm(`Discard changes to ${files.length} file(s)? This cannot be undone.`)) return;
    try { await gitDiscard(files, repoRef.current); await refreshStatus(); }
    catch (err) { console.error("Failed to discard:", err); onError?.("Failed to discard changes"); }
  }, [refreshStatus, onError]);

  // ── Commit ────────────────────────────────────────────

  const handleCommit = useCallback(async () => {
    if (!commitMsg.trim()) return;
    setCommitting(true);
    try {
      await gitCommit(commitMsg, repoRef.current);
      setCommitMsg("");
      await refreshStatus();
    } catch (err) {
      console.error("Failed to commit:", err);
      onError?.("Commit failed");
    } finally {
      setCommitting(false);
    }
  }, [commitMsg, refreshStatus, onError]);

  // ── Checkout ──────────────────────────────────────────

  const handleCheckout = useCallback(async (branchName: string) => {
    if (branchName === branch) return;
    setCheckingOut(true);
    try {
      const result = await gitCheckout(branchName, repoRef.current);
      if (result.success) {
        setBranch(branchName);
        refreshStatus();
      } else {
        onError?.(result.message || "Checkout failed");
      }
    } catch {
      onError?.("Failed to checkout branch");
    } finally {
      setCheckingOut(false);
    }
  }, [branch, setBranch, onError, refreshStatus]);

  return {
    commitMsg, setCommitMsg, committing,
    checkingOut,
    handleStage, handleUnstage, handleStageAll, handleUnstageAll,
    handleDiscard, handleCommit, handleCheckout,
  };
}
