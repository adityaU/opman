/**
 * Keybinding commands for the panel.
 *
 * The command ids are declared in `keybindings/commands/panels.ts` and bound in
 * the base and vim layers, so they are a published surface — a rebuilt panel
 * that stopped answering them would break bindings a user has already learned,
 * and silently, because an unhandled command is simply a key that does nothing.
 */

import { useCommands } from "../../keybindings/useCommand";
import type { GitSection } from "../types";
import type { GitSelection } from "./useGitSelection";

export interface GitCommandDeps {
  readonly setSection: (section: GitSection) => void;
  readonly selection: GitSelection;
  readonly refresh: () => void;
  readonly stage: (files: string[]) => void;
  readonly unstage: (files: string[]) => void;
  readonly discard: (files: string[]) => void;
  readonly stageAll: () => void;
  readonly unstageAll: () => void;
  readonly openDiff: (path: string, staged: boolean) => void;
  readonly focusCommitMessage: () => void;
  readonly commit: () => void;
  readonly openBranchPicker: () => void;
  readonly openRepoPicker: () => void;
  readonly generateCommitMessage: () => void;
  readonly sendToReview: () => void;
  readonly goBack: () => void;
}

export function useGitCommands(deps: GitCommandDeps): void {
  const { selection } = deps;

  /** Run against whatever row the keyboard cursor is on, or do nothing. */
  const onSelection =
    (run: (path: string, variant: string) => void) =>
    () => {
      const selected = selection.selected;
      if (selected) run(selected.path, selected.variant);
    };

  useCommands({
    "git.nextFile": selection.moveDown,
    "git.previousFile": selection.moveUp,

    // Stage and unstage stay separate rather than collapsing into one toggle:
    // an untracked file has no meaningful unstage, and a toggle would make the
    // same key do different things depending on where the cursor landed.
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
      deps.openDiff(path, variant === "staged");
    }),

    "git.changesTab": () => deps.setSection("changes"),
    "git.logTab": () => deps.setSection("history"),
    "git.refresh": deps.refresh,
    "git.stageAll": deps.stageAll,
    "git.unstageAll": deps.unstageAll,
    "git.focusCommitMessage": deps.focusCommitMessage,
    "git.commit": deps.commit,
    "git.switchBranch": deps.openBranchPicker,
    "git.switchRepo": deps.openRepoPicker,
    "git.generateCommitMessage": deps.generateCommitMessage,
    "git.sendToReview": deps.sendToReview,
    "git.back": deps.goBack,
  });
}
