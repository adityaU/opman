import { useRef, useState } from "react";
import {
  Check, Loader2, GitCommitHorizontal,
  MessageSquare, FileEdit, GitPullRequest,
} from "lucide-react";
import type { GitFileEntry, GitView } from "../types";
import { FileSection } from "./FileSection";

interface Props {
  loading: boolean;
  staged: GitFileEntry[];
  unstaged: GitFileEntry[];
  untracked: GitFileEntry[];
  commitMsg: string;
  setCommitMsg: (msg: string) => void;
  committing: boolean;
  handleCommit: () => void;
  handleStage: (files: string[]) => void;
  handleUnstage: (files: string[]) => void;
  handleStageAll: () => void;
  handleUnstageAll: () => void;
  handleDiscard: (files: string[]) => void;
  pushView: (v: GitView) => void;
  // AI actions
  onSendToAI?: (text: string) => void;
  aiReviewLoading: boolean;
  aiCommitMsgLoading: boolean;
  aiPrLoading: boolean;
  handleAIReview: () => void;
  handleAICommitMsg: () => void;
  openPRBranchPicker: () => void;
}

export function ChangesListView({
  loading, staged, unstaged, untracked,
  commitMsg, setCommitMsg, committing, handleCommit,
  handleStage, handleUnstage, handleStageAll, handleUnstageAll, handleDiscard,
  pushView, onSendToAI,
  aiReviewLoading, aiCommitMsgLoading, aiPrLoading,
  handleAIReview, handleAICommitMsg, openPRBranchPicker,
}: Props) {
  const commitInputRef = useRef<HTMLTextAreaElement>(null);
  const [stagedOpen, setStagedOpen]       = useState(true);
  const [unstagedOpen, setUnstagedOpen]   = useState(true);
  const [untrackedOpen, setUntrackedOpen] = useState(true);

  const totalChanges = staged.length + unstaged.length + untracked.length;

  const handleCommitKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") { e.preventDefault(); handleCommit(); }
  };

  if (loading) {
    return <div className="git-loading"><Loader2 size={18} className="spin" /></div>;
  }

  if (totalChanges === 0) {
    return <div className="git-empty"><Check size={20} /><span>Working tree clean</span></div>;
  }

  return (
    <>
      {/* AI action buttons */}
      {onSendToAI && (
        <div className="git-ai-actions">
          <button className="git-ai-button" onClick={handleAIReview}
            disabled={aiReviewLoading || (staged.length === 0 && unstaged.length === 0)}
            title="Send all changes to AI for code review"
          >
            {aiReviewLoading ? <Loader2 size={12} className="spin" /> : <MessageSquare size={12} />}
            Review Changes
          </button>
          <button className="git-ai-button" onClick={handleAICommitMsg}
            disabled={aiCommitMsgLoading || staged.length === 0}
            title="Generate a commit message from staged changes"
          >
            {aiCommitMsgLoading ? <Loader2 size={12} className="spin" /> : <FileEdit size={12} />}
            Write Commit Msg
          </button>
          <button className="git-ai-button" onClick={openPRBranchPicker}
            disabled={aiPrLoading} title="Draft a PR description from branch changes"
          >
            {aiPrLoading ? <Loader2 size={12} className="spin" /> : <GitPullRequest size={12} />}
            Draft PR
          </button>
        </div>
      )}

      {/* Commit form */}
      {staged.length > 0 && (
        <div className="git-commit-form">
          <textarea
            ref={commitInputRef} className="git-commit-input"
            placeholder="Commit message..." value={commitMsg}
            onChange={(e) => setCommitMsg(e.target.value)}
            onKeyDown={handleCommitKeyDown} rows={3}
          />
          <button className="git-commit-button" onClick={handleCommit}
            disabled={!commitMsg.trim() || committing}
          >
            {committing ? <Loader2 size={13} className="spin" /> : <GitCommitHorizontal size={13} />}
            Commit ({staged.length} staged)
          </button>
        </div>
      )}

      {/* File sections */}
      <FileSection
        title="Staged" files={staged} isOpen={stagedOpen}
        onToggle={() => setStagedOpen((v) => !v)} pushView={pushView}
        variant="staged" onBulkAction={handleUnstageAll} onFileAction={handleUnstage}
      />
      <FileSection
        title="Changes" files={unstaged} isOpen={unstagedOpen}
        onToggle={() => setUnstagedOpen((v) => !v)} pushView={pushView}
        variant="unstaged" onBulkAction={handleStageAll} onFileAction={handleStage}
        onDiscard={handleDiscard}
      />
      <FileSection
        title="Untracked" files={untracked} isOpen={untrackedOpen}
        onToggle={() => setUntrackedOpen((v) => !v)} pushView={pushView}
        variant="untracked" onBulkAction={handleStageAll} onFileAction={handleStage}
      />
    </>
  );
}
