import { useCallback, useMemo, useRef } from "react";
import { newSession, sendMessage, abortSession } from "../api";
import type { AppState, ImageAttachment } from "../api";
import type { Message, PermissionRequest, QuestionRequest } from "../types";
import type { PaneContext } from "./WorkspaceRoot";
import type { WorkspaceBridge, WorkspaceProject } from "./DesktopWorkspace";
import type { TargetRequest } from "./target/useTargeting";
import type { WorkspaceChatServices } from "./widgets/WorkspaceChatContext";
import type { PaneEngine, WidgetState } from "./types";

/**
 * Assembles what the desktop workspace needs out of the app's state.
 *
 * Kept out of `ChatLayout` because it is entirely derivable — every value here
 * is a projection of `appState` plus a handful of callbacks — and putting it in
 * the layout would grow a component that is already the largest in the app.
 */

export interface WorkspaceShellDeps {
  readonly appState: AppState | null;
  readonly busySessions: ReadonlySet<string>;
  /** The shell composer's engine — what a pane that has not chosen one uses. */
  readonly defaultEngine: PaneEngine;
  readonly availableRunners: string[];
  readonly openModelPicker: () => void;
  readonly openAgentPicker: () => void;
  readonly runSlashCommand: (command: string, args?: string) => Promise<void> | void;
  readonly onError: (message: string) => void;
  readonly subagentMessages: Map<string, Message[]>;
  readonly isBookmarked: (messageId: string) => boolean;
  readonly toggleBookmark: (
    messageId: string,
    sessionId: string,
    role: string,
    preview: string,
  ) => void;
  readonly openSession: (sessionId: string) => void;
  /** Every pending request, from any session — panes filter to their own. */
  readonly permissions: PermissionRequest[];
  readonly questions: QuestionRequest[];
  readonly onPermissionReply: (
    requestId: string,
    reply: "once" | "always" | "reject",
  ) => Promise<void>;
  readonly onQuestionReply: (requestId: string, answers: string[][]) => Promise<void>;
  readonly onQuestionDismiss: (requestId: string) => Promise<void>;
  readonly searchOpen: boolean;
  readonly closeSearch: () => void;
}

export function useWorkspaceShellProps(deps: WorkspaceShellDeps) {
  const { appState } = deps;

  // Read through a ref so the callbacks below keep a stable identity: they are
  // passed into an effect in DesktopWorkspace, and a new function per render
  // would re-publish on every keystroke anywhere in the app.
  const latest = useRef(deps);
  latest.current = deps;

  const projects = useMemo<WorkspaceProject[]>(
    () =>
      (appState?.projects ?? []).map((project: { path: string; name: string }) => ({
        path: project.path,
        name: project.name,
      })),
    [appState?.projects],
  );

  const projectIndexOf = useCallback(
    (path: string) => (latest.current.appState?.projects ?? []).findIndex((p: { path: string }) => p.path === path),
    [],
  );

  /**
   * The project's sessions, most recently touched first.
   *
   * The backend returns them in its own order and the opener was showing that
   * order verbatim, so the session you were working in a minute ago could sit
   * anywhere in the list. The sidebar has always sorted by `time.updated`;
   * this is the same comparator, so the two lists agree.
   */
  const sessionsFor = useCallback(
    (projectPath: string) => {
      const project = (latest.current.appState?.projects ?? []).find(
        (candidate: { path: string }) => candidate.path === projectPath,
      );
      const sessions = (project?.sessions ?? []) as {
        id: string;
        title?: string;
        time?: { updated?: number };
      }[];
      return sessions
        .map((session) => ({
          id: session.id,
          title: session.title || session.id.slice(0, 8),
          updated: session.time?.updated ?? 0,
        }))
        .sort((a, b) => b.updated - a.updated);
    },
    [],
  );

  /**
   * A pane's chrome: which project, what it is showing, and whether its agent
   * is working. Recomputed per render rather than memoised per pane — it reads
   * three fields and memoising per widget identity would cost more than it saves.
   *
   * Deliberately *not* read through `latest`, unlike its neighbours here. A
   * mounted window re-renders when this function's identity changes and at no
   * other time (see `WindowView`), so a ref would freeze every pane header's
   * subtitle and busy dot at whatever they were when the window was last
   * touched. These two inputs are the whole reason it may change.
   */
  const describe = useCallback(
    (widget: WidgetState | null): PaneContext => {
      if (!widget) return { projectName: "", subtitle: null, busy: false };
      const project = (appState?.projects ?? []).find(
        (candidate: { path: string }) => candidate.path === widget.projectPath,
      );
      const projectName = project?.name ?? basename(widget.projectPath);

      if (widget.kind !== "chat") {
        return { projectName, subtitle: null, busy: false };
      }
      const session = ((project?.sessions ?? []) as { id: string; title?: string }[]).find(
        (candidate) => candidate.id === widget.sessionId,
      );
      return {
        projectName,
        subtitle: widget.sessionId ? session?.title ?? widget.sessionId.slice(0, 8) : "New session",
        busy: widget.sessionId ? deps.busySessions.has(widget.sessionId) : false,
      };
    },
    [appState?.projects, deps.busySessions],
  );

  /**
   * Send, creating the session when the pane does not have one yet.
   *
   * The runner reaches the wire in exactly one of two ways: as the engine a new
   * session is created on, or — only when the pane says the user just switched
   * it — as a named runner on the message. Re-stating it on every send is what
   * used to read as a switch request and fork the conversation into a handoff.
   */
  const send = useCallback<WorkspaceChatServices["send"]>(
    async (target, text, images?: ImageAttachment[]) => {
      const { engine } = target;
      let sid = target.sessionId;
      if (!sid) {
        const index = projectIndexOf(target.projectPath);
        if (index < 0) {
          latest.current.onError("That project is no longer open");
          return { ok: false, sessionId: null };
        }
        try {
          sid = (await newSession(index, engine.runner || null)).session_id;
        } catch {
          latest.current.onError("Failed to create session");
          return { ok: false, sessionId: null };
        }
      }
      try {
        await sendMessage(
          sid,
          text,
          engine.model ?? undefined,
          images,
          engine.agent || undefined,
          // A session created moments ago is already on this runner; naming it
          // again would ask upstream to switch a session to what it already is.
          target.switchRunner && target.sessionId ? engine.runner : undefined,
          engine.effort ?? undefined,
          engine.permission || undefined,
        );
        return { ok: true, sessionId: sid };
      } catch {
        latest.current.onError("Failed to send message");
        // The session exists even though the send failed, so hand it back: the
        // pane should bind to it rather than create a second one on retry.
        return { ok: false, sessionId: sid };
      }
    },
    [projectIndexOf],
  );

  const chat = useMemo<Omit<WorkspaceChatServices, "bindSession" | "setEngine">>(
    () => ({
      appState,
      send,
      abort: async (sessionId: string) => {
        await abortSession(sessionId).catch(() => {});
      },
      runCommand: async (_sessionId: string, command: string, args?: string) => {
        await latest.current.runSlashCommand(command, args);
      },
      openModelPicker: () => latest.current.openModelPicker(),
      openAgentPicker: () => latest.current.openAgentPicker(),
      defaultEngine: deps.defaultEngine,
      availableRunners: deps.availableRunners,
      onError: (message: string) => latest.current.onError(message),
      subagentMessages: deps.subagentMessages,
      isBookmarked: (messageId: string) => latest.current.isBookmarked(messageId),
      toggleBookmark: (messageId, sessionId, role, preview) =>
        latest.current.toggleBookmark(messageId, sessionId, role, preview),
      openSession: (sessionId: string) => latest.current.openSession(sessionId),
      permissions: deps.permissions,
      questions: deps.questions,
      onPermissionReply: (requestId, reply) => latest.current.onPermissionReply(requestId, reply),
      onQuestionReply: (requestId, answers) => latest.current.onQuestionReply(requestId, answers),
      onQuestionDismiss: (requestId) => latest.current.onQuestionDismiss(requestId),
      searchOpen: deps.searchOpen,
      closeSearch: () => latest.current.closeSearch(),
    }),
    [
      appState,
      deps.availableRunners,
      deps.defaultEngine,
      deps.permissions,
      deps.questions,
      deps.searchOpen,
      deps.subagentMessages,
      send,
    ],
  );

  // The shell drives the workspace through this. A ref because the actions
  // arrive from a child after the first render, and because the callbacks that
  // close over it must not change identity when they do.
  const bridgeRef = useRef<WorkspaceBridge | null>(null);
  const onTargetingReady = useCallback((api: WorkspaceBridge | null) => {
    bridgeRef.current = api;
  }, []);

  /** Returns false when the workspace is not mounted, so callers can fall back. */
  const armTargeting = useCallback((request: TargetRequest) => {
    if (!bridgeRef.current) return false;
    bridgeRef.current.arm(request);
    return true;
  }, []);

  const openKindHere = useCallback((kind: "files" | "terminal" | "git") => {
    if (!bridgeRef.current) return false;
    bridgeRef.current.openKindHere(kind);
    return true;
  }, []);

  /** Returns false when the workspace is not mounted — mobile, or the board. */
  const openFileInWorkspace = useCallback((path: string, line: number | null) => {
    if (!bridgeRef.current) return false;
    bridgeRef.current.openFile(path, line);
    return true;
  }, []);

  return useMemo(
    () => ({
      workspaceProps: {
        projects,
        sessionsFor,
        describe,
        busySessions: deps.busySessions,
        chat,
        onError: (message: string) => latest.current.onError(message),
        onTargetingReady,
      },
      armTargeting,
      openKindHere,
      openFileInWorkspace,
    }),
    [armTargeting, openKindHere, openFileInWorkspace, chat, deps.busySessions, describe, onTargetingReady, projects, sessionsFor],
  );
}

function basename(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
