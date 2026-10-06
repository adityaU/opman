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
  /**
   * The heading this row sits under. Consecutive rows sharing one share the
   * heading — how projects are grouped by server once there is more than one.
   */
  readonly group?: string;
  /** Muted text beside the group heading: a server's status, say. */
  readonly groupNote?: string;
  /** Shown but not choosable — a server that cannot be reached right now. */
  readonly disabled?: boolean;
}

/**
 * A project choice names a server as well as a path once there is more than
 * one server, and a choice's value is one string. The own server's projects
 * keep the bare path, so a single-server opener answers exactly as it always
 * has; another server's are prefixed with a separator no path contains.
 */
const SERVER_MARK = "\u0001";

export function projectChoiceValue(server: string | null | undefined, path: string): string {
  return server ? `${SERVER_MARK}${server}${SERVER_MARK}${path}` : path;
}

export function decodeProjectChoice(value: string): { server: string | null; path: string } {
  if (!value.startsWith(SERVER_MARK)) return { server: null, path: value };
  const end = value.indexOf(SERVER_MARK, 1);
  if (end < 0) return { server: null, path: value };
  return { server: value.slice(1, end) || null, path: value.slice(end + 1) };
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
  /** The server the project is on; absent or null is this instance's own. */
  readonly server?: string | null;
}

export const EMPTY_DRAFT: OpenerDraft = {
  kind: null,
  projectPath: null,
  sessionId: undefined,
  ptyId: undefined,
  server: null,
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
    case "project": {
      if (value === null) return draft;
      const { server, path } = decodeProjectChoice(value);
      return { ...draft, projectPath: path, server };
    }
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
  if (draft.projectPath !== null) return { ...draft, projectPath: null, server: null };
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
  const widget = toLocalWidget(draft, paneId);
  if (!widget || !draft.server) return widget;
  return { ...widget, server: draft.server };
}

function toLocalWidget(draft: OpenerDraft, paneId?: PaneId): WidgetState | null {
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
