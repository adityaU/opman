/**
 * The widget opener's step machine.
 *
 * Kind, then project, then the thing that kind is *of* — a session for chat, a
 * shell for a terminal. Modelled as data rather than as nested components so
 * "which step am I on" is one value, going back is popping it, and the whole
 * flow is testable without rendering.
 *
 * The step list is derived, never stored: `files` and `git` finish at the
 * project, and hard-coding a length would mean remembering to change it when
 * another widget arrives.
 */

import { browserIdForProject } from "../../api/browser";
import { WIDGET_KINDS, type PaneId, type WidgetKind, type WidgetState } from "../types";

export type StepId = "kind" | "project" | "session" | "shell";

/**
 * One answerable option.
 *
 * Deliberately thin — a label and a hint, never a pre-rendered row. What a
 * choice *looks* like is the step's business (rows.tsx), so the caller that
 * knows the data does not also have to know the vocabulary.
 */
export interface OpenerChoice {
  readonly value: string | null;
  readonly label: string;
  readonly hint?: string;
  /** Marks a choice as actively working — a shell running a command. */
  readonly busy?: boolean;
}

export interface OpenerDraft {
  readonly kind: WidgetKind | null;
  readonly projectPath: string | null;
  /**
   * Three states, not two: `undefined` is "not asked yet", `null` is the real
   * answer "start a new session here", and a string names an existing one.
   * Collapsing the first two makes a chat draft look finished before the user
   * has chosen anything.
   */
  readonly sessionId: string | null | undefined;
  /**
   * Which running shell a terminal will show. Same three states as `sessionId`,
   * and for the same reason — `null` is the real answer "a new shell", which the
   * pane then starts, and is not the same as not having been asked.
   */
  readonly ptyId: string | null | undefined;
}

export const EMPTY_DRAFT: OpenerDraft = {
  kind: null,
  projectPath: null,
  sessionId: undefined,
  ptyId: undefined,
};

/** The last step is what the kind is *of*: a conversation, or a shell. */
export function stepsFor(kind: WidgetKind | null): readonly StepId[] {
  if (kind === null) return ["kind"];
  if (kind === "chat") return ["kind", "project", "session"];
  if (kind === "terminal") return ["kind", "project", "shell"];
  return ["kind", "project"];
}

/** The step awaiting an answer, or null when the draft is finished. */
export function currentStep(draft: OpenerDraft): StepId | null {
  if (draft.kind === null) return "kind";
  if (draft.projectPath === null) return "project";
  if (draft.kind === "chat" && draft.sessionId === undefined) return "session";
  if (draft.kind === "terminal" && draft.ptyId === undefined) return "shell";
  return null;
}

export function isComplete(draft: OpenerDraft): boolean {
  return currentStep(draft) === null;
}

/** Advance the draft with the answer to its current step. */
export function advance(draft: OpenerDraft, value: string | null): OpenerDraft {
  switch (currentStep(draft)) {
    case "kind": {
      if (value === null || !WIDGET_KINDS.includes(value as WidgetKind)) return draft;
      return { ...draft, kind: value as WidgetKind };
    }
    case "project":
      return value === null ? draft : { ...draft, projectPath: value };
    case "session":
      return { ...draft, sessionId: value };
    case "shell":
      return { ...draft, ptyId: value };
    case null:
      return draft;
  }
}

/**
 * Step back one, clearing what that step had answered.
 *
 * Driven by the last *answered* step rather than the current one, so it works
 * from a finished draft too — which is where the user most often wants it.
 */
export function retreat(draft: OpenerDraft): OpenerDraft {
  if (draft.kind === null) return draft;
  if (draft.kind === "chat" && draft.sessionId !== undefined) {
    return { ...draft, sessionId: undefined };
  }
  if (draft.kind === "terminal" && draft.ptyId !== undefined) {
    return { ...draft, ptyId: undefined };
  }
  if (draft.projectPath !== null) return { ...draft, projectPath: null };
  return { ...draft, kind: null };
}

/**
 * The widget a finished draft describes, or null if it is not finished.
 *
 * Building the union here is what keeps the illegal combinations unspellable
 * downstream: a terminal literally cannot carry the session id the draft may
 * still be holding from an abandoned chat branch.
 */
export function toWidget(draft: OpenerDraft, paneId?: PaneId): WidgetState | null {
  const { kind, projectPath } = draft;
  if (kind === null || projectPath === null || !isComplete(draft)) return null;

  switch (kind) {
    case "chat":
      return { kind: "chat", projectPath, sessionId: draft.sessionId ?? null, engine: null };
    case "files":
      return paneId ? { kind: "files", projectPath, sessionId: paneId, open: null } : null;
    case "terminal":
      // `null` here is the answer "a new shell": the pane starts one, because
      // only it knows the size to open it at.
      return { kind: "terminal", projectPath, ptyId: draft.ptyId ?? null };
    case "git":
      return { kind: "git", projectPath };
    case "browser":
      // Per project, not per pane: the id is derived from the project so a second
      // browser opened for the same repo reconnects to the tab already running
      // there — including one an agent opened.
      return {
        kind: "browser",
        projectPath,
        browserId: browserIdForProject(projectPath),
        url: null,
        reveal: 0,
      };
  }
}
