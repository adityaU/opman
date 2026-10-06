/**
 * What the opener offers at each step, and the widgets a choice turns into.
 *
 * Split from `useWorkspacePlacement`, which decides *where* things go; this is
 * only *what* there is to choose from. Pure apart from the inputs it is handed,
 * so the lists can be read without mounting a workspace.
 */

import { browserIdForProject } from "../../api/browser";
import type { PtySession } from "../../api";
import { formatTime } from "../../sidebar/formatTime";
import type { WorkspaceProject } from "../DesktopWorkspace";
import { REMOTE_SHELL_CHOICES } from "../servers/serverChoices";
import { WIDGET_KINDS, type PaneId, type WidgetKind, type WidgetState } from "../types";
import type { OpenerChoice, OpenerDraft, StepId } from "./steps";

export const KIND_LABEL: Readonly<Record<WidgetKind, string>> = {
  chat: "Chat",
  files: "Files",
  terminal: "Terminal",
  git: "Git",
  browser: "Browser",
};

export interface ChoiceSources {
  /** The project rows — flat with one server, grouped by server with several. */
  readonly projectChoices: readonly OpenerChoice[];
  readonly sessionsFor: (path: string) => readonly { id: string; title: string; updated: number }[];
  /** A project's sessions on another server. */
  readonly remoteSessions: (server: string, path: string) => readonly OpenerChoice[];
  readonly shells: readonly PtySession[];
}

export function choicesForStep(
  step: StepId,
  draft: OpenerDraft,
  sources: ChoiceSources,
): readonly OpenerChoice[] {
  if (step === "kind") {
    return WIDGET_KINDS.map((kind) => ({ value: kind, label: KIND_LABEL[kind] }));
  }
  if (step === "project") return sources.projectChoices;
  if (step === "shell") {
    return draft.server ? REMOTE_SHELL_CHOICES : shellChoices(sources.shells, draft.projectPath);
  }
  if (draft.server && draft.projectPath) return sources.remoteSessions(draft.server, draft.projectPath);
  // Recency-sorted by `sessionsFor`; "New session" is pinned above it
  // because it is the answer to a different question than "which one".
  const sessions = draft.projectPath ? sources.sessionsFor(draft.projectPath) : [];
  return [
    { value: null, label: "New session", hint: "created on first send" },
    ...sessions.map((s) => ({ value: s.id, label: s.title, hint: formatTime(s.updated) })),
  ];
}

export function widgetFor(kind: WidgetKind, projectPath: string, paneId: PaneId): WidgetState {
  switch (kind) {
    case "chat":
      return { kind: "chat", projectPath, sessionId: null, engine: null };
    case "files":
      return { kind: "files", projectPath, sessionId: paneId, open: null };
    case "terminal":
      // No shell chosen: the pane shows the picker, listing what is already
      // running here alongside "new shell".
      return { kind: "terminal", projectPath, ptyId: null };
    case "git":
      return { kind: "git", projectPath };
    case "browser":
      return {
        kind: "browser",
        projectPath,
        browserId: browserIdForProject(projectPath),
        url: null,
        reveal: 0,
      };
  }
}

/**
 * The running shells in one project, busiest first, under "New shell".
 *
 * Busy first because the shell someone is looking for is usually the one with
 * work in it; a numbered label alone gives no reason to prefer any of them.
 */
export function shellChoices(
  shells: readonly PtySession[],
  projectPath: string | null,
): readonly OpenerChoice[] {
  const mine = projectPath ? shells.filter((shell) => shell.project === projectPath) : [];
  const ordered = [...mine].sort((a, b) => {
    if (a.activity !== b.activity) return a.activity === "running" ? -1 : 1;
    return a.label.localeCompare(b.label, undefined, { numeric: true });
  });
  return [
    { value: null, label: "New shell", hint: "started in the project root" },
    ...ordered.map((shell) => ({
      value: shell.id,
      label: shell.label,
      hint: shell.activity === "running" ? "running a command" : "idle",
      busy: shell.activity === "running",
    })),
  ];
}

/**
 * "Chat · opman", and "Chat · opman · dev box" when the widget is on another
 * server — the overlay's label is the only place the destination is named
 * before it lands, and two servers can hold projects of the same name.
 */
export function describeWidget(
  widget: WidgetState,
  projects: readonly WorkspaceProject[],
  serverName: (id: string) => string,
): string {
  const own = widget.server ? undefined : projects.find((p) => p.path === widget.projectPath);
  const name = own?.name ?? (widget.server ? basenameOf(widget.projectPath) : widget.projectPath);
  const base = `${KIND_LABEL[widget.kind]} · ${name}`;
  return widget.server ? `${base} · ${serverName(widget.server)}` : base;
}

function basenameOf(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}
