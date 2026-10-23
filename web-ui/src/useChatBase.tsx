import { useState, useCallback, useMemo, useEffect, useRef } from "react";
import { useSSE } from "./hooks/useSSE";
import { useToast } from "./hooks/useToast";
import { useProviders } from "./hooks/useProviders";
import { useBookmarks } from "./hooks/useBookmarks";
import { useModalState } from "./hooks/useModalState";
import { useRunnerConfig } from "./hooks/useRunnerConfig";
import { useSessionSelection } from "./hooks/useSessionSelection";
import { useKeymapContext } from "./keybindings/KeymapContext";
import { defaultRunner } from "./chatSessionHandlers";
import { getPersistedThemeMode, applyThemeMode } from "./theme-selector/persistence";
import type { ThemeMode } from "./theme-selector/persistence";
import { getPersistedAppearance, initAppearance } from "./utils/appearance";
import type { Appearance } from "./utils/appearance";
import { useSettingsRoute } from "./settings-page";
import { useKanbanViewState } from "./kanban/useKanbanViewState";
import { useSessionTaskLinks } from "./sidebar/useSessionTaskLinks";
import { useServerPreferences } from "./hooks/useServerPreferences";

export function useChatBase() {
  // Latches once the app has painted for the first time; see the startup gate
  // below.
  const hasStartedRef = useRef(false);

  // A `/name` in the composer resolves to a command id and runs through the same registry
  // as its chord, so the two can never drift into separate implementations.
  const keymap = useKeymapContext();
  const { runCommand } = keymap;

  // ── Core SSE state ──
  const sse = useSSE();
  const {
    appState, messages, stats, busySessions, permissions, questions,
    crossSessionPermissions, crossSessionQuestions, sessionStatus,
    isLoadingMessages, isLoadingOlder, hasOlderMessages, totalMessageCount,
    watcherStatus, subagentMessages, fileEditCount,
    mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId, mcpBrowserOpen, mcpAgentActivity,
    refreshState, clearPermission, clearQuestion,
    clearMcpEditorOpen, clearMcpTerminalFocus, clearMcpBrowserOpen,
    addOptimisticMessage, clearOptimistic, loadOlderMessages, beginSessionSwitch,
    isSessionBusy,
  } = sse;

  // Stable serialized key — changes only when the actual set of busy IDs changes.
  // Consumed by sidebar children that need to re-render on busy state change,
  // while avoiding passing the raw Set (which gets a new reference every event).
  const busyKey = useMemo(() => {
    if (busySessions.size === 0) return "";
    return Array.from(busySessions).sort().join(",");
  }, [busySessions]);

  // ── Active session + project (single source of truth) ──
  const {
    sessionId: urlSessionId,
    projectIndex: urlProjectIndex,
    newSessionMode,
    selectSessionAt: setUrlSession,
    selectProject,
  } = useSessionSelection({ appState, beginSessionSwitch });

  // ── Derived app state ──
  // URL is the single source of truth for active session + project.
  // Fall back to server state only when URL has no session yet (initial load).
  // URL is the sole source of truth for the active project — never fall back to server state.
  const activeProjectIndex = urlProjectIndex;
  const activeProject = appState ? appState.projects[activeProjectIndex] ?? null : null;
  const activeSessionId = newSessionMode ? null : (urlSessionId ?? activeProject?.active_session ?? null);
  const activeSession = activeProject?.sessions?.find((session: any) => session.id === activeSessionId);

  // Build the set of sub-session IDs (children of the active session)
  const subSessionIds = useMemo(() => {
    if (!activeSessionId || !activeProject?.sessions) return new Set<string>();
    const ids = new Set<string>();
    for (const s of activeProject.sessions) {
      if (s.parentID === activeSessionId) ids.add(s.id);
    }
    return ids;
  }, [activeSessionId, activeProject?.sessions]);

  // Only include cross-session permissions/questions from sub-sessions (not unrelated sessions)
  const allPermissions = useMemo(
    () => [...permissions, ...crossSessionPermissions.filter((p) => subSessionIds.has(p.sessionID))],
    [permissions, crossSessionPermissions, subSessionIds],
  );
  const allQuestions = useMemo(
    () => [...questions, ...crossSessionQuestions],
    [questions, crossSessionQuestions],
  );

  // ── Theme ──
  const [themeMode, setThemeMode] = useState<ThemeMode>(getPersistedThemeMode);
  const [appearance, setAppearanceState] = useState<Appearance>(getPersistedAppearance);
  useEffect(() => {
    applyThemeMode(themeMode);
    initAppearance();
    // Note: fetchThemePair() is already called in useSSE on mount —
    // no need to duplicate it here.
  }, []);

  // ── Modals / Toast / Providers / Bookmarks ──
  useServerPreferences(setAppearanceState, setThemeMode);
  const modalState = useModalState({ onOpen: sse.blockSessionAdoption });
  const { toasts, addToast, removeToast } = useToast();
  // A runner pick belongs to the session it was made on: if it outlived that
  // session it would name a runner for the *next* one, which the backend reads
  // as a switch request and answers by forking a handoff session.
  // `isSwitch` records *when* the pick was made — choosing a runner before the
  // session exists configures it, while choosing one for a live session asks to
  // move that conversation. Only the second is a handoff request — and a session
  // whose runner label is briefly wrong (the label lands after creation) must
  // never turn the first kind into the second.
  const [runnerChoice, setRunnerChoice] = useState<
    { sessionId: string | null; runner: string; isSwitch: boolean } | null
  >(null);
  // What each runner was last configured with — model, agent, effort, permission
  // — kept per runner and across reloads, because none of those values mean
  // anything outside the runner they were chosen in.
  const runnerConfig = useRunnerConfig();
  const pickedRunner = runnerChoice && runnerChoice.sessionId === activeSessionId ? runnerChoice.runner : null;
  const availableRunners = useMemo<string[]>(
    () => appState?.runners || ["opencode", "claude-code", "claude", "codex"],
    [appState?.runners],
  );
  // A brand-new session has no runner of its own, and the server default is not
  // what the user was working in. Prefer their last pick — but only while it is
  // still on offer, so a renamed or removed ACP agent can never be sent.
  const rememberedRunner = runnerConfig.lastRunner();
  const fallbackRunner = availableRunners.includes(rememberedRunner)
    ? rememberedRunner
    : defaultRunner(appState);
  const currentRunner = pickedRunner || activeSession?.runner || fallbackRunner;
  const runnerSwitch = pickedRunner && runnerChoice?.isSwitch && pickedRunner !== activeSession?.runner
    ? pickedRunner
    : null;
  const clearRunnerChoice = useCallback(() => setRunnerChoice(null), []);
  /** Follow the pick onto the session it produced, without re-arming a switch. */
  const bindRunnerChoice = useCallback(
    (sessionId: string, runner: string) => setRunnerChoice({ sessionId, runner, isSwitch: false }),
    [],
  );
  const providers = useProviders(currentRunner);
  const { isBookmarked, toggleBookmark } = useBookmarks();

  // ── Settings page (its own route: /settings) ──
  const settings = useSettingsRoute();
  // Leaving settings returns to the conversation that was open, or the chat root.
  const leaveSettings = useCallback(() => {
    if (activeSessionId) setUrlSession(activeSessionId, activeProjectIndex);
    else selectProject(activeProjectIndex);
  }, [activeSessionId, activeProjectIndex, setUrlSession, selectProject]);

  // ── Kanban board view (its own route: /kanban) ──
  const {
    isKanbanView, boardProjectIndex, focusTaskId,
    openKanban, openKanbanTask, setBoardProject, clearFocusTask,
  } = useKanbanViewState();
  // Leaving the board returns to chat ("/") for the project the board was on —
  // honouring an in-board project switch. Prefer the session we came from when
  // it's the same project, else that project's backend-active session.
  const goToChat = useCallback(() => {
    const proj = boardProjectIndex ?? activeProjectIndex;
    const sid =
      (proj === activeProjectIndex ? activeSessionId : null) ??
      appState?.projects?.[proj]?.active_session ??
      null;
    if (sid) setUrlSession(sid, proj);
    else selectProject(proj);
  }, [boardProjectIndex, activeProjectIndex, activeSessionId, appState, setUrlSession, selectProject]);
  const toggleKanbanView = useCallback(() => {
    if (isKanbanView) goToChat();
    else openKanban(activeProjectIndex);
  }, [isKanbanView, goToChat, openKanban, activeProjectIndex]);
  // Sidebar back-link opens the task on the active project's board.
  const handleOpenKanbanTask = useCallback(
    (taskId: string) => openKanbanTask(taskId, activeProjectIndex),
    [openKanbanTask, activeProjectIndex],
  );

  // Reverse map (session → originating kanban task/lane) for the active project,
  // so the sidebar can tag kanban-launched sessions and link back to their task.
  const sessionTaskLinks = useSessionTaskLinks(activeProjectIndex, activeProject?.path);

  return {
    hasStartedRef, keymap, runCommand, sse, appState, messages,
    stats, busySessions, permissions, questions, crossSessionPermissions, crossSessionQuestions,
    sessionStatus, isLoadingMessages, isLoadingOlder, hasOlderMessages, totalMessageCount, watcherStatus,
    subagentMessages, fileEditCount, mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId, mcpBrowserOpen,
    mcpAgentActivity, refreshState, clearPermission, clearQuestion, clearMcpEditorOpen, clearMcpTerminalFocus,
    clearMcpBrowserOpen, addOptimisticMessage, clearOptimistic, loadOlderMessages, beginSessionSwitch, isSessionBusy,
    busyKey, urlSessionId, urlProjectIndex, newSessionMode, setUrlSession, selectProject,
    activeProjectIndex, activeProject, activeSessionId, activeSession, subSessionIds, allPermissions,
    allQuestions, themeMode, setThemeMode, appearance, setAppearanceState, modalState,
    toasts, addToast, removeToast, runnerChoice, setRunnerChoice, runnerConfig,
    pickedRunner, availableRunners, rememberedRunner, fallbackRunner, currentRunner, runnerSwitch,
    clearRunnerChoice, bindRunnerChoice, providers, isBookmarked, toggleBookmark, settings,
    leaveSettings, isKanbanView, boardProjectIndex, focusTaskId, openKanban, openKanbanTask,
    setBoardProject, clearFocusTask, goToChat, toggleKanbanView, handleOpenKanbanTask, sessionTaskLinks,
  };
}
