import { useCommands } from "../keybindings/useCommand";
import type { GitTab, GitView } from "./types";
import type { GitSelection } from "./useGitSelection";

/**
 * Registers the git panel's commands.
 *
 * Row-level actions read the keyboard selection, so `s` stages whatever `j` and
 * `k` last landed on. Stage and unstage are separate commands rather than one
 * toggle: an untracked file has no meaningful "unstage", and a toggle would
 * make `u` do something different depending on where the cursor happens to be.
 */

export interface GitCommandDeps {
  readonly setTab: (tab: GitTab) => void;
  readonly refreshStatus: () => void;
  readonly stageAll: () => void;
  readonly unstageAll: () => void;
  readonly expandAllFiles: () => void;
  readonly collapseAllFiles: () => void;
  readonly generateCommitMessage: () => void;
  readonly sendToReview: () => void;
  readonly createPr: () => void;
  readonly goBack: () => void;
  readonly selection: GitSelection;
  readonly stage: (files: string[]) => void;
  readonly unstage: (files: string[]) => void;
  readonly discard: (files: string[]) => void;
  readonly pushView: (view: GitView) => void;
}

export function useGitCommands(deps: GitCommandDeps): void {
  const { selection } = deps;

  const onSelection = (run: (path: string, variant: string) => void) => () => {
    const selected = selection.selected;
    if (selected) run(selected.path, selected.variant);
  };

  useCommands({
    "git.nextFile": selection.moveDown,
    "git.previousFile": selection.moveUp,
    "git.stageFile": onSelection((path, variant) => {
      if (variant !== "staged") deps.stage([path]);
    }),
    "git.unstageFile": onSelection((path, variant) => {
      if (variant === "staged") deps.unstage([path]);
    }),
    "git.toggleStageFile": onSelection((path, variant) => {
      if (variant === "staged") deps.unstage([path]);
      else deps.stage([path]);
    }),
    // Untracked files have nothing to discard back to.
    "git.discard": onSelection((path, variant) => {
      if (variant === "unstaged") deps.discard([path]);
    }),
    "git.openDiff": onSelection((path, variant) => {
      if (variant === "untracked") return;
      deps.pushView({ kind: "file-diff", file: path, staged: variant === "staged" });
    }),
    "git.changesTab": () => deps.setTab("changes"),
    "git.logTab": () => deps.setTab("log"),
    "git.refresh": deps.refreshStatus,
    "git.stageAll": deps.stageAll,
    "git.unstageAll": deps.unstageAll,
    "git.expandAll": deps.expandAllFiles,
    "git.collapseAll": deps.collapseAllFiles,
    "git.generateCommitMessage": deps.generateCommitMessage,
    "git.sendToReview": deps.sendToReview,
    "git.createPr": deps.createPr,
    "git.back": deps.goBack,
  });
}
