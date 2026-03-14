import { useState, useCallback, useRef } from "react";
import { fetchGitDiff, fetchGitRangeDiff } from "../../api";
import type { GitFileEntry, PRModalData } from "../types";

/**
 * AI-powered git actions: review, commit message generation, PR description.
 * All operations forward the selected repo path to the API.
 */
export function useAIActions(
  staged: GitFileEntry[],
  unstaged: GitFileEntry[],
  selectedRepo: string | undefined,
  onSendToAI?: (text: string) => void,
  onError?: (msg: string) => void,
) {
  const [aiReviewLoading, setAiReviewLoading]       = useState(false);
  const [aiCommitMsgLoading, setAiCommitMsgLoading] = useState(false);
  const [aiPrLoading, setAiPrLoading]               = useState(false);
  const [prModalOpen, setPrModalOpen]                 = useState(false);
  const [prModalData, setPrModalData]                 = useState<PRModalData | null>(null);

  const repoRef = useRef(selectedRepo);
  repoRef.current = selectedRepo;

  // ── Review changes ────────────────────────────────────

  const handleAIReview = useCallback(async () => {
    if (!onSendToAI) return;
    setAiReviewLoading(true);
    try {
      // Quick check that there are actual changes to review
      const [stagedDiff, unstagedDiff] = await Promise.all([
        staged.length > 0  ? fetchGitDiff(undefined, true, repoRef.current)  : Promise.resolve({ diff: "" }),
        unstaged.length > 0 ? fetchGitDiff(undefined, false, repoRef.current) : Promise.resolve({ diff: "" }),
      ]);
      const combined = [stagedDiff.diff, unstagedDiff.diff].filter(Boolean).join("\n");
      if (!combined.trim()) { onError?.("No changes to review"); return; }

      const stagedCount = staged.length;
      const unstagedCount = unstaged.length;
      const fileSummary = [
        stagedCount > 0 ? `${stagedCount} staged` : "",
        unstagedCount > 0 ? `${unstagedCount} unstaged` : "",
      ].filter(Boolean).join(", ");

      onSendToAI(
        `Review my current git changes (${fileSummary} files). Use the available tools to read the git diff, then provide a thorough code review covering potential bugs, code quality, and suggestions for improvement.`,
      );
    } catch (err) {
      console.error("Failed to gather diff for AI review:", err);
      onError?.("Failed to gather changes for review");
    } finally {
      setAiReviewLoading(false);
    }
  }, [onSendToAI, staged.length, unstaged.length, onError]);

  // ── Write commit message ──────────────────────────────

  const handleAICommitMsg = useCallback(async () => {
    if (!onSendToAI) return;
    if (staged.length === 0) { onError?.("Stage some files first"); return; }
    setAiCommitMsgLoading(true);
    try {
      const { diff } = await fetchGitDiff(undefined, true, repoRef.current);
      if (!diff.trim()) { onError?.("No staged changes found"); return; }
      const fileList = staged.map((f) => `  ${f.status} ${f.path}`).join("\n");
      onSendToAI(
        `Generate a concise, well-structured git commit message for the following staged changes.\n\nStaged files:\n${fileList}\n\nDiff:\n\`\`\`diff\n${diff}\n\`\`\`\n\nWrite a commit message following conventional commit format (e.g. feat:, fix:, refactor:). Include a brief subject line (max 72 chars) and an optional body with bullet points if needed. Return ONLY the commit message, nothing else.`,
      );
    } catch (err) {
      console.error("Failed to gather diff for commit message:", err);
      onError?.("Failed to gather staged changes");
    } finally {
      setAiCommitMsgLoading(false);
    }
  }, [onSendToAI, staged, onError]);

  // ── Draft PR description ──────────────────────────────

  /** Whether we're in the "pick a base branch" step before drafting the PR. */
  const [prBranchPickerOpen, setPrBranchPickerOpen] = useState(false);

  /** Open the branch picker step (called by the Draft PR button). */
  const openPRBranchPicker = useCallback(() => {
    setPrBranchPickerOpen(true);
  }, []);

  /** Actually draft the PR with a chosen base branch. */
  const handleAIPRDescription = useCallback(async (baseBranch?: string) => {
    if (!onSendToAI) return;
    setPrBranchPickerOpen(false);
    setAiPrLoading(true);
    try {
      const rangeData = await fetchGitRangeDiff(baseBranch || undefined, undefined, repoRef.current);
      if (!rangeData.diff.trim() && rangeData.commits.length === 0) {
        onError?.("No commits found relative to base branch");
        return;
      }
      setPrModalData(rangeData);
      setPrModalOpen(true);
      const commitList = rangeData.commits
        .map((c) => `  - ${c.hash.slice(0, 7)} ${c.message}`)
        .join("\n");
      onSendToAI(
        `Draft a pull request description for merging \`${rangeData.branch}\` into \`${rangeData.base}\`.\n\nCommits (${rangeData.commits.length}):\n${commitList}\n\nFiles changed: ${rangeData.files_changed}\n\nDiff:\n\`\`\`diff\n${rangeData.diff.slice(0, 8000)}${rangeData.diff.length > 8000 ? "\n... (diff truncated)" : ""}\n\`\`\`\n\nWrite a clear PR description with:\n- A concise title\n- A summary section with bullet points\n- Any notable changes or breaking changes\n- Testing notes if applicable`,
      );
    } catch (err) {
      console.error("Failed to gather range diff for PR description:", err);
      onError?.("Failed to gather branch changes");
    } finally {
      setAiPrLoading(false);
    }
  }, [onSendToAI, onError]);

  return {
    aiReviewLoading, aiCommitMsgLoading, aiPrLoading,
    prModalOpen, setPrModalOpen, prModalData,
    prBranchPickerOpen, setPrBranchPickerOpen, openPRBranchPicker,
    handleAIReview, handleAICommitMsg, handleAIPRDescription,
  };
}
