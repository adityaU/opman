import React, { useCallback, useMemo, useState } from "react";
import { MessageTimeline } from "../../MessageTimeline";
import { PromptInput } from "../../PromptInput";
import { PermissionDock } from "../../PermissionDock";
import { QuestionDock } from "../../QuestionDock";
import { SearchBar } from "../../SearchBar";
import { useSessionView } from "../../hooks/sse/useSessionView";
import { loadOlderIn } from "../../hooks/sse/sessionStore";
import { SESSION_BUSY, SESSION_IDLE } from "../../hooks/sse/types";
import { useWorkspaceChat } from "./WorkspaceChatContext";
import { usePaneEngine } from "./usePaneEngine";
import type { ImageAttachment } from "../../api";
import type { PaneEngine } from "../types";
import { activeProgressText } from "../../prompt-input/progress";

/**
 * A chat pane.
 *
 * Reads its transcript from the per-session store rather than from `useSSE`'s
 * single `messages` array, which is the whole reason more than one of these can
 * be live at once. The pane names the session; nothing here consults the
 * "active" session, so two panes on two different conversations are symmetric —
 * neither is the real one.
 *
 * That symmetry is why the interaction docks live inside the pane: a permission
 * belongs to the conversation that asked for it, and with two conversations on
 * screen there is no "the" dock to put it in.
 */

export interface ChatWidgetProps {
  readonly paneId: string;
  readonly projectPath: string;
  /** Null means "a new session here", created lazily by the first send. */
  readonly sessionId: string | null;
  /** Null while the pane follows the shell's engine rather than owning one. */
  readonly engine: PaneEngine | null;
  /** Only the focused pane shows the shell's find-in-transcript. */
  readonly focused: boolean;
}

export const ChatWidget: React.FC<ChatWidgetProps> = function ChatWidget({
  paneId,
  projectPath,
  sessionId,
  engine,
  focused,
}) {
  const services = useWorkspaceChat();
  const view = useSessionView(sessionId);
  const controls = usePaneEngine(paneId, engine, sessionId);
  const [sending, setSending] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [matches, setMatches] = useState<SearchMatches>(NO_MATCHES);

  const busy = view.status.type !== "idle";

  const onSend = useCallback(
    async (text: string, images?: ImageAttachment[]): Promise<boolean> => {
      if (sending) return false;
      setSending(true);
      try {
        const target = {
          sessionId,
          projectPath,
          engine: controls.engine,
          switchRunner: controls.switchRunner,
        };
        const result = await services.send(target, text, images);
        if (controls.switchRunner) controls.runnerSent();
        // A "new session" pane learns its id from its own first send. Without
        // this the pane would still be sessionless on the next reload and the
        // conversation it started would be unreachable from here.
        if (result.ok && !sessionId && result.sessionId) {
          services.bindSession(paneId, result.sessionId);
        }
        return result.ok;
      } finally {
        setSending(false);
      }
    },
    [controls, paneId, projectPath, sending, services, sessionId],
  );

  const onAbort = useCallback(async () => {
    if (sessionId) await services.abort(sessionId);
  }, [services, sessionId]);

  const onCommand = useCallback(
    async (command: string, args?: string) => {
      if (sessionId) await services.runCommand(sessionId, command, args);
    },
    [services, sessionId],
  );

  const onLoadOlder = useCallback(async (): Promise<boolean> => {
    if (!sessionId || loadingOlder) return false;
    setLoadingOlder(true);
    try {
      return await loadOlderIn(sessionId);
    } finally {
      setLoadingOlder(false);
    }
  }, [loadingOlder, sessionId]);

  // Only this pane's conversation. A permission raised by the session in the
  // next pane belongs over there, not on top of this transcript.
  const permissions = useMemo(
    () => services.permissions.filter((request) => request.sessionID === sessionId),
    [services.permissions, sessionId],
  );
  const questions = useMemo(
    () => services.questions.filter((request) => request.sessionID === sessionId),
    [services.questions, sessionId],
  );

  const searchOpen = focused && services.searchOpen;
  const closeSearch = useCallback(() => {
    setMatches(NO_MATCHES);
    services.closeSearch();
  }, [services]);
  const onMatchesChanged = useCallback(
    (matchIds: Set<string>, activeId: string | null) => setMatches({ matchIds, activeId }),
    [],
  );

  return (
    <div className="wsp-chat" data-surface="chat" data-pane-chat={paneId}>
      {searchOpen && (
        <SearchBar
          messages={view.messages as never[]}
          onClose={closeSearch}
          onMatchesChanged={onMatchesChanged}
        />
      )}

      <MessageTimeline
        messages={view.messages as never[]}
        sessionStatus={busy ? SESSION_BUSY : SESSION_IDLE}
        activeSessionId={sessionId}
        isSending={sending}
        isLoadingMessages={view.loading}
        isLoadingOlder={loadingOlder}
        hasOlderMessages={view.hasOlder}
        totalMessageCount={view.total}
        onLoadOlder={onLoadOlder}
        appState={services.appState}
        onSendPrompt={onSend}
        subagentMessages={services.subagentMessages}
        searchMatchIds={matches.matchIds}
        activeSearchMatchId={matches.activeId}
        isBookmarked={services.isBookmarked}
        onToggleBookmark={services.toggleBookmark}
        onOpenSession={services.openSession}
      />

      {permissions.length > 0 && (
        <PermissionDock
          permissions={permissions}
          activeSessionId={sessionId}
          onReply={services.onPermissionReply}
          onGoToSession={services.openSession}
        />
      )}
      {questions.length > 0 && (
        <QuestionDock
          questions={questions}
          activeSessionId={sessionId}
          onReply={services.onQuestionReply}
          onDismiss={services.onQuestionDismiss}
          onGoToSession={services.openSession}
        />
      )}

      <PromptInput
        onSend={onSend}
        onAbort={onAbort}
        onCommand={onCommand}
        onOpenModelPicker={services.openModelPicker}
        onOpenAgentPicker={services.openAgentPicker}
        isBusy={busy}
        isSending={sending}
        // Never disabled on a null session: a new-session pane has to accept
        // the first prompt, which is what creates the session.
        disabled={!services.appState}
        sessionId={sessionId}
        stats={view.stats}
        progressText={activeProgressText(view.messages, busy)}
        currentModel={controls.engine.model?.modelID ?? null}
        selectedModel={controls.engine.model}
        onModelSelected={controls.setModel}
        currentAgent={controls.engine.agent}
        onAgentChange={controls.setAgent}
        currentRunner={controls.engine.runner}
        availableRunners={services.availableRunners}
        onRunnerChange={controls.setRunner}
        effort={controls.engine.effort}
        onEffortChange={controls.setEffort}
        permission={controls.engine.permission}
        onPermissionChange={controls.setPermission}
        backend={services.appState?.backend}
      />
    </div>
  );
};

interface SearchMatches {
  readonly matchIds: Set<string>;
  readonly activeId: string | null;
}

const NO_MATCHES: SearchMatches = { matchIds: new Set(), activeId: null };
