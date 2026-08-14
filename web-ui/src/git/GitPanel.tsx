/**
 * The git panel shell.
 *
 * Layout is a fixed masthead — identity, in-flight operation, section nav —
 * over one scrolling body. The masthead never scrolls away because the two
 * things a person needs before acting are which branch they are on and
 * whether the repository is mid-merge.
 */

import { useCallback, useMemo, useState } from "react";

import { GitHeader } from "./components/GitHeader";
import { ActionResult } from "./components/ActionResult";
import { OperationBanner } from "./components/OperationBanner";
import { SectionNav } from "./components/SectionNav";
import { GitPanelProvider, useGitBridge } from "./state/GitPanelContext";
import { useGitAction } from "./state/useGitAction";
import { useGitCommands } from "./state/useGitCommands";
import { useGitData } from "./state/useGitData";
import { useGitRepos } from "./state/useGitRepos";
import { useGitSelection } from "./state/useGitSelection";
import { asAction } from "./components/gitFormat";
import * as api from "./api";
import type { GitSection } from "./types";
import { BranchesView } from "./views/BranchesView";
import { ChangesView } from "./views/ChangesView";
import { HistoryView } from "./views/HistoryView";
import { StashesView } from "./views/StashesView";
import { WorktreesView } from "./views/WorktreesView";

/** Shared empty list, so an absent status does not remount the selection. */
const EMPTY: never[] = [];

export interface GitPanelProps {
  focused?: boolean;
  projectPath: string;
  onError?: (message: string) => void;
  onSendToAI?: (prompt: string) => void;
}

export function GitPanel({ focused, projectPath, onSendToAI }: GitPanelProps) {
  const repos = useGitRepos(projectPath);
  const data = useGitData(repos.scope);
  const [section, setSection] = useState<GitSection>("changes");

  const refreshAll = useCallback(async () => {
    await Promise.all([data.refresh(), repos.refresh()]);
  }, [data.refresh, repos.refresh]);

  const action = useGitAction(refreshAll);

  const selection = useGitSelection(
    data.status?.staged ?? EMPTY,
    data.status?.unstaged ?? EMPTY,
    data.status?.untracked ?? EMPTY,
  );

  const bridge = useGitBridge(selection);

  useGitCommands({
    setSection,
    selection,
    refresh: () => void refreshAll(),
    stage: (files) =>
      void action.run("stage", asAction("Staged", () => api.stage(repos.scope, files))),
    unstage: (files) =>
      void action.run("unstage", asAction("Unstaged", () => api.unstage(repos.scope, files))),
    discard: (files) =>
      void action.run("discard", asAction("Discarded", () => api.discard(repos.scope, files))),
    stageAll: () =>
      void action.run("stage", asAction("Staged all", () => api.stage(repos.scope, []))),
    unstageAll: () =>
      void action.run("unstage", asAction("Unstaged all", () => api.unstage(repos.scope, []))),
    // The rest need a widget only a view owns, so they route through the bridge.
    openDiff: (path, staged) => {
      setSection("changes");
      bridge.invoke("openDiff", path, staged);
    },
    focusCommitMessage: () => {
      setSection("changes");
      bridge.invoke("focusCommitMessage");
    },
    commit: () => bridge.invoke("commit"),
    openBranchPicker: () => bridge.invoke("openBranchPicker"),
    openRepoPicker: () => bridge.invoke("openRepoPicker"),
    generateCommitMessage: () => bridge.invoke("generateCommitMessage"),
    sendToReview: () => bridge.invoke("sendToReview"),
    goBack: () => bridge.invoke("goBack"),
  });

  const counts = useMemo(
    () => ({
      changes:
        (data.status?.staged.length ?? 0) +
        (data.status?.unstaged.length ?? 0) +
        (data.status?.untracked.length ?? 0),
      branches: data.branches?.local.length ?? 0,
      worktrees: data.worktrees.length,
      stashes: data.stashes.length,
    }),
    [data.status, data.branches, data.worktrees, data.stashes],
  );

  if (data.error) {
    return (
      <div className="gitp" data-surface="git">
        <p className="gitp-fatal" role="alert">
          <span className="gitp-fatal-title">This folder could not be read as a git repository.</span>
          <span className="gitp-fatal-detail">{data.error}</span>
          <button type="button" className="gitp-btn" onClick={() => void refreshAll()}>
            Try again
          </button>
        </p>
      </div>
    );
  }

  const body = {
    changes: <ChangesView data={data} scope={repos.scope} action={action} onSendToAI={onSendToAI} />,
    history: <HistoryView data={data} scope={repos.scope} action={action} />,
    branches: <BranchesView data={data} scope={repos.scope} action={action} />,
    worktrees: <WorktreesView data={data} scope={repos.scope} action={action} />,
    stashes: <StashesView data={data} scope={repos.scope} action={action} />,
  }[section];

  return (
    <GitPanelProvider value={bridge}>
      <div className="gitp" data-surface="git" data-focused={focused ? "" : undefined}>
        <GitHeader repos={repos} data={data} action={action} />

        {data.operation ? (
          <OperationBanner operation={data.operation} scope={repos.scope} action={action} />
        ) : null}

        <SectionNav section={section} onChange={setSection} counts={counts} />

        {action.result ? <ActionResult result={action.result} onDismiss={action.dismiss} /> : null}

        <div className="gitp-body">
          {data.loading && !data.status ? <GitSkeleton /> : body}
        </div>
      </div>
    </GitPanelProvider>
  );
}

/**
 * A shape-matched placeholder rather than a spinner: the panel keeps its
 * geometry as data arrives, so nothing jumps under the pointer.
 */
function GitSkeleton() {
  return (
    <div className="gitp-skeleton" aria-hidden="true">
      {[0, 1, 2, 3, 4].map((row) => (
        <div key={row} className="gitp-skeleton-row" />
      ))}
    </div>
  );
}

export default GitPanel;
