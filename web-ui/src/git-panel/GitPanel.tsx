/**
 * GitPanel — web-native Git UI replacing the gitui PTY panel.
 *
 * This is the orchestrator component. All logic is delegated to:
 * - hooks: useViewNavigation, useGitData, useGitActions, useAIActions
 * - components: BreadcrumbNav, BranchSwitcher, GitTabBar,
 *               ChangesListView, LogListView, FileDiffView,
 *               CommitDetailView, PRModal
 */
import { useState } from "react";
import type { GitPanelProps, GitTab } from "./types";
import { buildDiffStyles } from "./utils";
import { useViewNavigation } from "./hooks/useViewNavigation";
import { useGitData } from "./hooks/useGitData";
import { useGitActions } from "./hooks/useGitActions";
import { useAIActions } from "./hooks/useAIActions";
import { BreadcrumbNav } from "./components/BreadcrumbNav";
import { BranchSwitcher } from "./components/BranchSwitcher";
import { GitTabBar } from "./components/GitTabBar";
import { ChangesListView } from "./components/ChangesListView";
import { LogListView } from "./components/LogListView";
import { FileDiffView } from "./components/FileDiffView";
import { CommitDetailView } from "./components/CommitDetailView";
import { PRModal } from "./components/PRModal";

export default function GitPanel({ projectPath, onError, onSendToAI }: GitPanelProps) {
  const [tab, setTab] = useState<GitTab>("changes");

  const nav = useViewNavigation();
  const data = useGitData(projectPath, tab, nav.currentView, onError);
  const actions = useGitActions(data.branch, data.setBranch, data.refreshStatus, onError);
  const ai = useAIActions(data.staged, data.unstaged, onSendToAI, onError);

  const diffStyles = buildDiffStyles(data.themeColors);
  const totalChanges = data.staged.length + data.unstaged.length + data.untracked.length;

  return (
    <div className="git-panel">
      {/* Toolbar: breadcrumbs + branch + refresh + tabs */}
      <div className="git-panel-header">
        <div className="git-panel-toolbar-row">
          <BreadcrumbNav
            viewStack={nav.viewStack} tab={tab}
            breadcrumbDropdown={nav.breadcrumbDropdown}
            setBreadcrumbDropdown={nav.setBreadcrumbDropdown}
            popView={nav.popView} jumpToView={nav.jumpToView}
          />
          <BranchSwitcher
            branch={data.branch} checkingOut={actions.checkingOut}
            loading={data.loading} logLoading={data.logLoading} tab={tab}
            localBranches={data.localBranches} remoteBranches={data.remoteBranches}
            branchesLoading={data.branchesLoading} fetchBranchList={data.fetchBranchList}
            handleCheckout={actions.handleCheckout}
            refreshStatus={data.refreshStatus} refreshLog={data.refreshLog}
          />
        </div>
        {nav.currentView.kind === "list" && (
          <GitTabBar tab={tab} setTab={setTab} totalChanges={totalChanges} resetStack={nav.resetStack} />
        )}
      </div>

      {/* Changes list */}
      {nav.currentView.kind === "list" && tab === "changes" && (
        <div className="git-changes-list">
          <ChangesListView
            loading={data.loading}
            staged={data.staged} unstaged={data.unstaged} untracked={data.untracked}
            commitMsg={actions.commitMsg} setCommitMsg={actions.setCommitMsg}
            committing={actions.committing} handleCommit={actions.handleCommit}
            handleStage={actions.handleStage} handleUnstage={actions.handleUnstage}
            handleStageAll={actions.handleStageAll} handleUnstageAll={actions.handleUnstageAll}
            handleDiscard={actions.handleDiscard} pushView={nav.pushView}
            onSendToAI={onSendToAI}
            aiReviewLoading={ai.aiReviewLoading} aiCommitMsgLoading={ai.aiCommitMsgLoading}
            aiPrLoading={ai.aiPrLoading}
            handleAIReview={ai.handleAIReview} handleAICommitMsg={ai.handleAICommitMsg}
            handleAIPRDescription={ai.handleAIPRDescription}
          />
        </div>
      )}

      {/* Log list */}
      {nav.currentView.kind === "list" && tab === "log" && (
        <div className="git-log-view">
          <LogListView logLoading={data.logLoading} commits={data.commits} pushView={nav.pushView} />
        </div>
      )}

      {/* File diff */}
      {nav.currentView.kind === "file-diff" && (
        <FileDiffView
          currentView={nav.currentView} diffOld={data.diffOld}
          diffNew={data.diffNew} diffLoading={data.diffLoading}
          diffStyles={diffStyles}
        />
      )}

      {/* Commit detail */}
      {nav.currentView.kind === "commit" && (
        <div className="git-commit-detail">
          <CommitDetailView
            commitDetail={data.commitDetail} commitDetailLoading={data.commitDetailLoading}
            expandedFiles={data.expandedFiles} toggleFileAccordion={data.toggleFileAccordion}
            expandAllFiles={data.expandAllFiles} collapseAllFiles={data.collapseAllFiles}
            diffStyles={diffStyles}
          />
        </div>
      )}

      {/* PR modal */}
      {ai.prModalOpen && ai.prModalData && (
        <PRModal data={ai.prModalData} onClose={() => ai.setPrModalOpen(false)} />
      )}
    </div>
  );
}
