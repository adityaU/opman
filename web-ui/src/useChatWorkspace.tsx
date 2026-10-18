import { useState, useCallback, useMemo, useEffect } from "react";
import { useShellFollowHandoff } from "./hooks/useShellFollowHandoff";
import { buildCommandHandlers } from "./chatLayoutCommands";
import { useCommands, useWhenContext } from "./keybindings/useCommand";
import type { FileOpenRequest } from "./code-editor/types";
import { isMobileViewport } from "./hooks/useIsMobile";
import { useWorkspaceShellProps } from "./workspace/useWorkspaceShellProps";
import { useBrowserLinks } from "./browser-panel/useBrowserLinks";
import type { useChatSession } from "./useChatSession";

export function useChatWorkspace(context: ReturnType<typeof useChatSession>) {
  const {
    keymap, appState, busySessions, permissions, questions, crossSessionPermissions,
    crossSessionQuestions, subagentMessages, mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId, mcpBrowserOpen,
    clearMcpEditorOpen, clearMcpTerminalFocus, clearMcpBrowserOpen, activeProjectIndex, activeProject, activeSessionId,
    allPermissions, modalState, addToast, availableRunners, currentRunner, isBookmarked,
    toggleBookmark, isKanbanView, openKanban, sidebar, mobile, model,
    sessionEngine, openModal, handlers, openModelPicker, openAgentPicker,
  } = context;
  // ── Desktop workspace ──
  // What a pane that has never chosen an engine of its own sends on.
  const defaultEngine = useMemo(
    () => ({
      runner: currentRunner,
      model: model.selectedModel,
      agent: model.selectedAgent,
      effort: sessionEngine.effort,
      permission: sessionEngine.permission,
    }),
    [currentRunner, sessionEngine.effort, sessionEngine.permission, model.selectedAgent, model.selectedModel],
  );

  // Unfiltered, unlike `allPermissions`: that list is scoped to the active
  // session and its subagents, and a pane may be showing neither. Each pane
  // filters to its own session.
  const paneInteractions = useMemo(
    () => ({
      permissions: [...permissions, ...crossSessionPermissions],
      questions: [...questions, ...crossSessionQuestions],
    }),
    [crossSessionPermissions, crossSessionQuestions, permissions, questions],
  );

  const {
    workspaceProps,
    armTargeting,
    openKindHere,
    openFileInWorkspace,
    openFileWhereInWorkspace,
    openBrowserInWorkspace,
  } = useWorkspaceShellProps({
    appState,
    activeSessionId,
    busySessions,
    defaultEngine,
    availableRunners,
    openModelPicker,
    openAgentPicker,
    runSlashCommand: (command: string, args?: string) => handlers.handleCommand(command, args),
    onError: (message: string) => addToast(message, "error"),
    onHandoff: (runner: string | null) => addToast(`Session handed off to ${runner || "new runner"}`, "success"),
    subagentMessages,
    isBookmarked,
    toggleBookmark,
    openSession: (sessionId: string) => handlers.handleSelectSession(sessionId, activeProjectIndex),
    permissions: paneInteractions.permissions,
    questions: paneInteractions.questions,
    onPermissionReply: handlers.handlePermissionReply,
    onQuestionReply: handlers.handleQuestionReply,
    onQuestionDismiss: handlers.handleQuestionDismiss,
    searchOpen: modalState.modals.searchBar,
    closeSearch: () => modalState.close("searchBar"),
  });

  /**
   * Reveal a file, from a tool-card path click or from the MCP editor-open
   * event.
   *
   * On desktop the workspace answers: it reuses the files pane already on
   * screen, or splits one in beside the pane you clicked from. Everywhere else
   * — mobile, the board — there is one editor surface, and this is the request
   * it reads. `seq` rises on every ask so clicking the same path twice, having
   * browsed elsewhere in between, reveals it twice.
   */
  const [fileOpen, setFileOpen] = useState<FileOpenRequest | null>(null);
  const openFileInEditor = useCallback(
    (path: string, line?: number | null) => {
      if (openFileInWorkspace(path, line ?? null)) return;
      setFileOpen((previous) => ({ path, line: line ?? null, seq: (previous?.seq ?? 0) + 1 }));
      if (isMobileViewport() && mobile.activePanel !== "editor") mobile.togglePanel("editor");
    },
    [mobile, openFileInWorkspace],
  );

  /**
   * The editor's own jumps, which offer a pane instead of choosing one. Falls
   * back to the ordinary reveal wherever the workspace is not mounted — mobile,
   * or the board — so the link still works, it just stops asking.
   */
  const editorOpener = useMemo(
    () => ({
      open: openFileInEditor,
      openWhere: (path: string, line: number | null, label: string) => {
        if (openFileWhereInWorkspace(path, line, label)) return;
        openFileInEditor(path, line);
      },
    }),
    [openFileInEditor, openFileWhereInWorkspace],
  );

  // The MCP `web_editor` tool asking for a file is the same request arriving
  // over SSE, so it takes the same route. Cleared on arrival: it is an event,
  // and a latched one would re-fire on the next unrelated render.
  useEffect(() => {
    if (!mcpEditorOpenPath) return;
    openFileInEditor(mcpEditorOpenPath, mcpEditorOpenLine);
    clearMcpEditorOpen();
  }, [mcpEditorOpenPath, mcpEditorOpenLine, openFileInEditor, clearMcpEditorOpen]);

  // Likewise for `web_terminal`: the shell it names lives in the workspace now,
  // so all this can do is make sure a terminal is on screen.
  useEffect(() => {
    if (!mcpTerminalFocusId) return;
    openKindHere("terminal");
    clearMcpTerminalFocus();
  }, [mcpTerminalFocusId, openKindHere, clearMcpTerminalFocus]);

  useBrowserLinks(activeProject?.path ?? "", openBrowserInWorkspace);

  // An agent drove a project's browser somewhere: show it. The workspace reuses
  // the pane already holding that project's browser, so this only ever creates
  // one when there is none — no second view of the same tab.
  useEffect(() => {
    if (!mcpBrowserOpen) return;
    window.dispatchEvent(new CustomEvent("opman:browser-agent-page", { detail: mcpBrowserOpen }));
    openBrowserInWorkspace(mcpBrowserOpen.projectPath, mcpBrowserOpen.url);
    clearMcpBrowserOpen();
  }, [mcpBrowserOpen, openBrowserInWorkspace, clearMcpBrowserOpen]);

  /**
   * Hand a chat to a pane instead of navigating. Reports false when it could
   * not, so the caller falls back to the old single-transcript navigation:
   * mobile, the board, or a workspace that has not mounted.
   */
  const targetChat = useCallback(
    (sessionId: string | null, projectIdx: number, label: string) => {
      if (isMobileViewport() || isKanbanView) return false;
      const project = appState?.projects?.[projectIdx];
      if (!project) return false;
      return armTargeting({
        widget: { kind: "chat", projectPath: project.path, sessionId, engine: null },
        label,
      });
    },
    [appState, armTargeting, isKanbanView],
  );

  /**
   * Clicking a session in the sidebar.
   *
   * On desktop it arms the pane-target overlay instead of navigating: the
   * sidebar no longer commands one transcript, it hands a session to whichever
   * pane the user picks.
   */
  const handleSelectSessionOrTarget = useCallback(
    (sessionId: string, projectIdx: number) => {
      const project = appState?.projects?.[projectIdx];
      const session = project?.sessions?.find((candidate: any) => candidate.id === sessionId);
      if (targetChat(sessionId, projectIdx, session?.title || sessionId.slice(0, 8))) return;
      handlers.handleSelectSession(sessionId, projectIdx);
    },
    [appState, handlers, targetChat],
  );

  useShellFollowHandoff(activeSessionId, activeProjectIndex, handlers.handleSelectSession);

  /**
   * Starting a new session — the same gesture, one step earlier. It has no id
   * to hand over yet, so the pane takes an unbound chat and creates the session
   * on first send; asking "which pane" only for existing sessions made the
   * button the one place in the workspace that seized a pane without asking.
   */
  const handleNewSessionOrTarget = useCallback(async () => {
    if (targetChat(null, activeProjectIndex, "New session")) return;
    await handlers.handleNewSession();
  }, [activeProjectIndex, handlers, targetChat]);
  /**
   * An agent-manager tool card asking to open the session it just started.
   *
   * The card knows an id and nothing else, so the lookup is here: find the
   * project that holds it, then take the same route the sidebar takes — arm the
   * pane overlay on desktop, navigate where there are no panes.
   */
  const projectIndexOfSession = useCallback(
    (sessionId: string) =>
      (appState?.projects ?? []).findIndex((project: any) =>
        project.sessions?.some((session: any) => session.id === sessionId),
      ),
    [appState?.projects],
  );
  const agentSessionOpener = useMemo(
    () => ({
      canOpen: (sessionId: string) => projectIndexOfSession(sessionId) >= 0,
      open: (sessionId: string, label: string) => {
        const index = projectIndexOfSession(sessionId);
        if (index < 0) return;
        if (targetChat(sessionId, index, label)) return;
        handlers.handleSelectSession(sessionId, index);
      },
    }),
    [handlers, projectIndexOfSession, targetChat],
  );

  const openMemoryActive = useCallback(() => modalState.openMemoryActive(), [modalState]);
  const openMemoryAll = useCallback(() => modalState.openMemoryAll(), [modalState]);
  const openCmdPalette = useCallback(() => modalState.open("commandPalette"), [modalState]);
  const closeSearchBar = useCallback(() => modalState.close("searchBar"), [modalState]);
  const openWatcher = useCallback(() => modalState.open("watcher"), [modalState]);
  const openCtxWindow = useCallback(() => modalState.open("contextWindow"), [modalState]);
  const onCompactCtx = useCallback(() => addToast("Compacting conversation...", "info"), [addToast]);

  // ── Keyboard: commands, not chords ──
  // The keymap owns which key runs what; this only says what the app can do.
  useCommands(buildCommandHandlers({
    openModal: modalState.open,
    toggleModal: modalState.toggle,
    closeTopModal: modalState.closeTopModal,
    toggleSidebar: sidebar.toggle,
    // The three panel chords open that widget in the focused pane. They no-op
    // where the workspace is not mounted — on mobile, whose dock owns those
    // surfaces, and on the board, which has no panes at all.
    toggleTerminal: () => { openKindHere("terminal"); },
    toggleEditor: () => { openKindHere("files"); },
    toggleGit: () => { openKindHere("git"); },
    toggleBoard: openKanban,
    newSession: handleNewSessionOrTarget,
    abortSession: handlers.handleAbort,
    copyTranscript: handlers.handleCopyTranscript,
    forwardToRunner: handlers.handleCommand,
    openMemoryActive,
    openMemoryAll,
    reloadApp: () => window.location.reload(),
  }));

  // Context keys read by `when` clauses. Every binding scoped to one of these
  // is inert until the matching surface says it applies. The editor, terminal
  // and git keys are published by the workspace instead: they describe the
  // focused pane, which only the workspace can see.
  useWhenContext({
    sessionActive: Boolean(activeSessionId),
    sessionBusy: model.sending,
    boardOpen: isKanbanView,
  });

  return {
    ...context,
    defaultEngine, paneInteractions, workspaceProps, armTargeting, openKindHere, openFileInWorkspace,
    openFileWhereInWorkspace, openBrowserInWorkspace, fileOpen, setFileOpen, openFileInEditor, editorOpener,
    targetChat, handleSelectSessionOrTarget, handleNewSessionOrTarget, projectIndexOfSession, agentSessionOpener, openMemoryActive,
    openMemoryAll, openCmdPalette, closeSearchBar, openWatcher, openCtxWindow, onCompactCtx,
  };
}
