import React, { createContext, useContext } from "react";
import type { AppState, ImageAttachment } from "../../api";
import type { Message, PermissionRequest, QuestionRequest } from "../../types";
import type { PaneEngine } from "../types";

/**
 * App-level services a chat pane needs but should not own.
 *
 * Provided once by the shell instead of threaded through the pane tree as
 * props. The tree is recursive and a pane may be four levels deep, so a prop
 * chain would make every intermediate component re-render on any change to
 * something it does not use — and would put the shell's whole surface area into
 * `PaneTree`'s signature, which is exactly what this directory is trying not to
 * know about.
 */

/** Where a send is going, and on what. */
export interface SendTarget {
  readonly sessionId: string | null;
  readonly projectPath: string;
  readonly engine: PaneEngine;
  /**
   * Whether to name the runner on the wire.
   *
   * Only true when the user has changed this pane's runner since its last send.
   * Naming a runner on every send to an existing session reads upstream as a
   * switch request and forks the conversation into a handoff, which is how a
   * second message used to land in a different session than the first.
   */
  readonly switchRunner: boolean;
}

export interface WorkspaceChatServices {
  readonly appState: AppState | null;
  /**
   * Send to a session, creating one in `projectPath` when `sessionId` is null.
   * Resolves to the session that received it, so the caller can bind a pane to
   * a session its own first send brought into existence.
   */
  readonly send: (
    target: SendTarget,
    text: string,
    images?: ImageAttachment[],
  ) => Promise<{ ok: boolean; sessionId: string | null }>;
  readonly abort: (sessionId: string) => Promise<void>;
  readonly runCommand: (sessionId: string, command: string, args?: string) => Promise<void>;
  /** The command palette's `/models` and `/agent` entry points. */
  readonly openModelPicker: () => void;
  readonly openAgentPicker: () => void;
  /** What a pane that has never chosen an engine of its own sends on. */
  readonly defaultEngine: PaneEngine;
  readonly availableRunners: string[];
  /** Give a pane an engine of its own. Persisted with the layout. */
  readonly setEngine: (paneId: string, engine: PaneEngine) => void;
  /** Bind a pane to a session created lazily by its first send. */
  readonly bindSession: (paneId: string, sessionId: string) => void;
  readonly onError: (message: string) => void;

  // ── Timeline extras, shared across every pane ──
  /** Transcripts of subagent sessions, keyed by session id. */
  readonly subagentMessages: Map<string, Message[]>;
  readonly isBookmarked: (messageId: string) => boolean;
  readonly toggleBookmark: (
    messageId: string,
    sessionId: string,
    role: string,
    preview: string,
  ) => void;
  /** Open a session in the pane that is asking — used by transcript back-links. */
  readonly openSession: (sessionId: string) => void;

  // ── Interaction requests, filtered per pane by session ──
  readonly permissions: PermissionRequest[];
  readonly questions: QuestionRequest[];
  readonly onPermissionReply: (
    requestId: string,
    reply: "once" | "always" | "reject",
  ) => Promise<void>;
  readonly onQuestionReply: (requestId: string, answers: string[][]) => Promise<void>;
  readonly onQuestionDismiss: (requestId: string) => Promise<void>;

  /** Whether the shell's find-in-transcript is open; the focused pane owns it. */
  readonly searchOpen: boolean;
  readonly closeSearch: () => void;
}

const WorkspaceChatContext = createContext<WorkspaceChatServices | null>(null);

export const WorkspaceChatProvider: React.FC<{
  value: WorkspaceChatServices;
  children: React.ReactNode;
}> = ({ value, children }) => (
  <WorkspaceChatContext.Provider value={value}>{children}</WorkspaceChatContext.Provider>
);

export function useWorkspaceChat(): WorkspaceChatServices {
  const services = useContext(WorkspaceChatContext);
  if (!services) {
    throw new Error("useWorkspaceChat used outside WorkspaceChatProvider");
  }
  return services;
}
