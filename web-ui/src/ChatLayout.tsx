import React, { Suspense, lazy, useState, useCallback, useMemo, useEffect, useRef } from "react";
import { useSSE } from "./hooks/useSSE";
import { useKeyboard } from "./hooks/useKeyboard";
import { useResizable } from "./hooks/useResizable";
import { useToast } from "./hooks/useToast";
import { useUrlState, readUrlState } from "./hooks/useUrlState";
import type { UrlState } from "./hooks/useUrlState";
import { useProviders } from "./hooks/useProviders";
import { ChatSidebar } from "./ChatSidebar";
import { MessageTimeline } from "./MessageTimeline";
import { PromptInput } from "./PromptInput";
import { StatusBar } from "./StatusBar";
import { CommandPalette } from "./CommandPalette";
import { ModelPickerModal } from "./ModelPickerModal";
import { AgentPickerModal } from "./AgentPickerModal";
import { PermissionDock } from "./PermissionDock";
import { QuestionDock } from "./QuestionDock";
import { TerminalPanel } from "./TerminalPanel";
import { XtermPanel } from "./XtermPanel";
const CodeEditorPanel = lazy(() => import("./CodeEditorPanel"));
const GitPanel = lazy(() => import("./GitPanel"));
import { ToastContainer } from "./ToastContainer";
import {
  ThemeSelectorModal,
  getPersistedThemeMode,
  applyThemeMode,
} from "./ThemeSelectorModal";
import type { ThemeMode } from "./ThemeSelectorModal";
import { CheatsheetModal } from "./CheatsheetModal";
import { TodoPanelModal } from "./TodoPanelModal";
import { SessionSelectorModal } from "./SessionSelectorModal";
import { ContextInputModal } from "./ContextInputModal";
import { SettingsModal } from "./SettingsModal";
import { WatcherModal } from "./WatcherModal";
import { ContextWindowPanel } from "./ContextWindowPanel";
import { DiffReviewPanel } from "./DiffReviewPanel";
import { SearchBar } from "./SearchBar";
import { CrossSessionSearchModal } from "./CrossSessionSearchModal";
import { SplitView } from "./SplitView";
const SessionGraph = lazy(() => import("./SessionGraph").then((m) => ({ default: m.SessionGraph })));
const SessionDashboard = lazy(() => import("./SessionDashboard").then((m) => ({ default: m.SessionDashboard })));
const ActivityFeed = lazy(() => import("./ActivityFeed").then((m) => ({ default: m.ActivityFeed })));
const NotificationPrefsModal = lazy(() => import("./NotificationPrefsModal").then((m) => ({ default: m.NotificationPrefsModal })));
const InboxModal = lazy(() => import("./InboxModal").then((m) => ({ default: m.InboxModal })));
const AutonomyModal = lazy(() => import("./AutonomyModal").then((m) => ({ default: m.AutonomyModal })));
const MemoryModal = lazy(() => import("./MemoryModal").then((m) => ({ default: m.MemoryModal })));
const RoutinesModal = lazy(() => import("./RoutinesModal").then((m) => ({ default: m.RoutinesModal })));
const DelegationBoardModal = lazy(() => import("./DelegationBoardModal").then((m) => ({ default: m.DelegationBoardModal })));
const AssistantCenterModal = lazy(() => import("./AssistantCenterModal").then((m) => ({ default: m.AssistantCenterModal })));
const MissionsModal = lazy(() => import("./MissionsModal").then((m) => ({ default: m.MissionsModal })));
const WorkspaceManagerModal = lazy(() => import("./WorkspaceManagerModal").then((m) => ({ default: m.WorkspaceManagerModal })));
import {
  getClientId,
  loadNotificationPrefs,
  showNotification,
} from "./NotificationManager";
import type { NotifyEventKind } from "./NotificationManager";
import type { AssistantSignal } from "./inbox";
import { buildResumeBriefing } from "./resumeBriefing";
import { buildDailySummary, buildRoutineSummary } from "./dailySummary";
import { buildRecommendations } from "./recommendations";
import {
  sendMessage,
  abortSession,
  executeCommand,
  replyPermission,
  replyQuestion,
  selectSession,
  newSession,
  switchProject,
  fetchTheme,
  registerPresence,
  deregisterPresence,
  fetchPersonalMemory,
  fetchAutonomySettings,
  updateAutonomySettings,
  fetchMissions,
  fetchRoutines,
  fetchDelegatedWork,
  fetchWorkspaces,
  runRoutine,
  createRoutine,
  saveWorkspace,
} from "./api";
import type { ThemeColors, ModelRef, ImageAttachment, PersonalMemoryItem, AutonomyMode, Mission, RoutineDefinition, RoutineRunRecord, DelegatedWorkItem, WorkspaceSnapshot } from "./api";
import { applyThemeToCss } from "./utils/theme";
import { useBookmarks } from "./hooks/useBookmarks";
import { X, FileCode, GitBranch, PanelLeft, Terminal, Command, MessageCircle, Sparkles, PenSquare } from "lucide-react";

/** Commands handled locally by the web UI instead of proxying to opencode */
const LOCAL_COMMANDS = new Set([
  "new",
  "terminal",
  "models",
  "model",
  "theme",
  "neovim",
  "nvim",
  "git",
  "agent",
  "keys",
  "keybindings",
  "todos",
  "sessions",
  "context",
  "settings",
  "watcher",
  "context-window",
  "diff-review",
  "search",
  "cross-search",
  "split-view",
  "session-graph",
  "session-dashboard",
  "activity-feed",
  "notification-prefs",
  "assistant-center",
  "inbox",
  "memory",
  "autonomy",
  "routines",
  "delegation",
  "missions",
  "workspaces",
]);

export function ChatLayout() {
  const sse = useSSE();
  const {
    appState,
    messages,
    stats,
    busySessions,
    permissions,
    questions,
    crossSessionPermissions,
    crossSessionQuestions,
    sessionStatus,
    isLoadingMessages,
    isLoadingOlder,
    hasOlderMessages,
    totalMessageCount,
    watcherStatus,
    subagentMessages,
    fileEditCount,
    mcpEditorOpenPath,
    mcpEditorOpenLine,
    mcpTerminalFocusId,
    mcpAgentActivity,
    refreshState,
    clearPermission,
    clearQuestion,
    clearMcpEditorOpen,
    clearMcpTerminalFocus,
    addOptimisticMessage,
    loadOlderMessages,
  } = sse;

  // Read initial URL state once on mount for panel defaults
  const [initialUrlState] = useState(() => readUrlState());

  const [sidebarOpen, setSidebarOpen] = useState(initialUrlState.panels.sidebar);
  const [terminalOpen, setTerminalOpen] = useState(initialUrlState.panels.terminal);
  const [neovimOpen, setNeovimOpen] = useState(initialUrlState.panels.editor);
  const [gitOpen, setGitOpen] = useState(initialUrlState.panels.git);

  // Track whether panels have ever been opened so they stay mounted (hidden)
  // once first shown. This preserves terminal shells, editor state, git state, etc.
  const [terminalMounted, setTerminalMounted] = useState(initialUrlState.panels.terminal);
  const [editorMounted, setEditorMounted] = useState(initialUrlState.panels.editor);
  const [gitMounted, setGitMounted] = useState(initialUrlState.panels.git);

  // Mark panels as mounted when first opened
  useEffect(() => { if (terminalOpen) setTerminalMounted(true); }, [terminalOpen]);
  useEffect(() => { if (neovimOpen) setEditorMounted(true); }, [neovimOpen]);
  useEffect(() => { if (gitOpen) setGitMounted(true); }, [gitOpen]);

  // MCP: auto-open editor panel when AI agent opens a file
  useEffect(() => {
    if (mcpEditorOpenPath) {
      setNeovimOpen(true);
      setEditorMounted(true);
      // Clear after a tick so CodeEditorPanel picks up the path first
      const timer = setTimeout(() => clearMcpEditorOpen(), 100);
      return () => clearTimeout(timer);
    }
  }, [mcpEditorOpenPath, clearMcpEditorOpen]);

  // MCP: auto-open/focus terminal panel when AI agent focuses a terminal
  useEffect(() => {
    if (mcpTerminalFocusId) {
      setTerminalOpen(true);
      setTerminalMounted(true);
      // Clear after handling
      clearMcpTerminalFocus();
    }
  }, [mcpTerminalFocusId, clearMcpTerminalFocus]);

  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [agentPickerOpen, setAgentPickerOpen] = useState(false);
  const [themeSelectorOpen, setThemeSelectorOpen] = useState(false);
  const [cheatsheetOpen, setCheatsheetOpen] = useState(false);
  const [todoPanelOpen, setTodoPanelOpen] = useState(false);
  const [sessionSelectorOpen, setSessionSelectorOpen] = useState(false);
  const [contextInputOpen, setContextInputOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [watcherOpen, setWatcherOpen] = useState(false);
  const [contextWindowOpen, setContextWindowOpen] = useState(false);
  const [diffReviewOpen, setDiffReviewOpen] = useState(false);
  const [searchBarOpen, setSearchBarOpen] = useState(false);
  const [crossSearchOpen, setCrossSearchOpen] = useState(false);
  const [splitViewOpen, setSplitViewOpen] = useState(false);
  const [splitViewSecondaryId, setSplitViewSecondaryId] = useState<string | null>(null);
  const [sessionGraphOpen, setSessionGraphOpen] = useState(false);
  const [sessionDashboardOpen, setSessionDashboardOpen] = useState(false);
  const [activityFeedOpen, setActivityFeedOpen] = useState(false);
  const [notificationPrefsOpen, setNotificationPrefsOpen] = useState(false);
  const [assistantCenterOpen, setAssistantCenterOpen] = useState(false);
  const [inboxOpen, setInboxOpen] = useState(false);
  const [memoryOpen, setMemoryOpen] = useState(false);
  const [autonomyOpen, setAutonomyOpen] = useState(false);
  const [routinesOpen, setRoutinesOpen] = useState(false);
  const [delegationOpen, setDelegationOpen] = useState(false);
  const [missionsOpen, setMissionsOpen] = useState(false);
  const [workspaceManagerOpen, setWorkspaceManagerOpen] = useState(false);
  const [assistantSignals, setAssistantSignals] = useState<AssistantSignal[]>([]);
  const [personalMemory, setPersonalMemory] = useState<PersonalMemoryItem[]>([]);
  const [autonomyMode, setAutonomyMode] = useState<AutonomyMode>("observe");
  const [missionCache, setMissionCache] = useState<Mission[]>([]);
  const [routineCache, setRoutineCache] = useState<RoutineDefinition[]>([]);
  const [routineRunCache, setRoutineRunCache] = useState<RoutineRunRecord[]>([]);
  const [delegatedWorkCache, setDelegatedWorkCache] = useState<DelegatedWorkItem[]>([]);
  const [workspaceCache, setWorkspaceCache] = useState<WorkspaceSnapshot[]>([]);
  const [executedAutoRoutineIds, setExecutedAutoRoutineIds] = useState<string[]>([]);
  const [resumeBriefing, setResumeBriefing] = useState<ReturnType<typeof buildResumeBriefing> | null>(null);
  const [latestDailySummary, setLatestDailySummary] = useState<string | null>(null);
  const [activeWorkspaceName, setActiveWorkspaceName] = useState<string | null>(null);
  const [searchMatchIds, setSearchMatchIds] = useState<Set<string>>(new Set());
  const [activeSearchMatchId, setActiveSearchMatchId] = useState<string | null>(null);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  /** Active full-screen panel on mobile (null = show chat/OpenCode) */
  const [activeMobilePanel, setActiveMobilePanel] = useState<
    "opencode" | "git" | "editor" | "terminal" | null
  >(null);
  /** Track which mobile panels have been visited so they stay mounted */
  const [mobilePanelsMounted, setMobilePanelsMounted] = useState<Set<string>>(
    new Set()
  );
  const [selectedModel, setSelectedModel] = useState<ModelRef | null>(null);
  const [selectedAgent, setSelectedAgent] = useState("");
  const [sending, setSending] = useState(false);
  const [themeMode, setThemeMode] = useState<ThemeMode>(getPersistedThemeMode);

  // ── Mobile input autohide state ──
  const [mobileInputHidden, setMobileInputHidden] = useState(false);
  const hasPromptContentRef = useRef(false);
  const mobileInputDebounceRef = useRef(0);

  /** Which panel currently has focus — used for unfocused dimming */
  const [focusedPanel, setFocusedPanel] = useState<"sidebar" | "chat" | "side">("chat");
  const lastVisibleAtRef = useRef(Date.now());

  // Toast notifications
  const { toasts, addToast, removeToast } = useToast();

  // Provider data for default model display
  const providers = useProviders();

  // Bookmarks
  const { isBookmarked, toggleBookmark, getSessionBookmarks, allBookmarks } = useBookmarks();

  // Resizable sidebar
  const sidebarResize = useResizable({
    initialSize: 280,
    minSize: 200,
    maxSize: 500,
  });

  // Resizable side panel (neovim/git)
  const sidePanelResize = useResizable({
    initialSize: 500,
    minSize: 300,
    maxSize: 900,
    reverse: true,
  });

  // Resizable terminal (vertical)
  const terminalResize = useResizable({
    initialSize: 250,
    minSize: 120,
    maxSize: 600,
    direction: "vertical",
    reverse: true,
  });

  // Load theme on mount
  useEffect(() => {
    applyThemeMode(themeMode);
    fetchTheme().then((colors) => {
      if (colors) applyThemeToCss(colors);
    });
  }, []);

  const activeProject = appState
    ? appState.projects[appState.active_project] ?? null
    : null;
  const activeSessionId = activeProject?.active_session ?? null;

  // Merge cross-session permissions/questions into the dock arrays so subagent
  // requests from child sessions surface in the parent session's UI.
  const allPermissions = useMemo(
    () => [...permissions, ...crossSessionPermissions],
    [permissions, crossSessionPermissions]
  );
  const allQuestions = useMemo(
    () => [...questions, ...crossSessionQuestions],
    [questions, crossSessionQuestions]
  );

  const activeMemoryItems = useMemo(
    () =>
      personalMemory.filter((item) => {
        if (!appState) return item.scope === "global";
        if (item.scope === "global") return true;
        if (item.scope === "project") return item.project_index === appState.active_project;
        return item.session_id === activeSessionId;
      }),
    [personalMemory, appState, activeSessionId]
  );

  // ── Assistant Pulse: top recommendation for ambient status bar chip ──
  const assistantPulse = useMemo(
    () => {
      const recs = buildRecommendations({
        autonomyMode,
        missions: missionCache,
        delegatedWork: delegatedWorkCache,
        memoryItems: activeMemoryItems,
        routines: routineCache,
        permissions,
        questions,
        workspaces: workspaceCache,
      });
      return recs.length > 0 ? recs[0] : null;
    },
    [autonomyMode, missionCache, delegatedWorkCache, activeMemoryItems, routineCache, permissions, questions, workspaceCache]
  );

  const handleRunAssistantPulse = useCallback(() => {
    if (!assistantPulse) return;
    switch (assistantPulse.action) {
      case "open_inbox":
        setInboxOpen(true);
        break;
      case "open_missions":
        setMissionsOpen(true);
        break;
      case "open_memory":
        setMemoryOpen(true);
        break;
      case "open_routines":
        setRoutinesOpen(true);
        break;
      case "open_delegation":
        setDelegationOpen(true);
        break;
      case "open_workspaces":
        setWorkspaceManagerOpen(true);
        break;
      case "open_autonomy":
        setAutonomyOpen(true);
        break;
      case "setup_daily_summary":
        createRoutine({
          name: "Daily Briefing",
          trigger: "daily_summary",
          action: "open_inbox",
          mission_id: null,
          session_id: null,
        })
          .then((routine) => {
            setRoutineCache((prev) => [routine, ...prev]);
            addToast("Daily briefing enabled", "success");
          })
          .catch(() => addToast("Failed to enable daily briefing", "error"));
        break;
      case "upgrade_autonomy_nudge":
        setAutonomyMode("nudge");
        updateAutonomySettings("nudge")
          .then(() => addToast("Autonomy set to Nudge", "success"))
          .catch(() => addToast("Failed to update autonomy", "error"));
        break;
      case "setup_daily_copilot":
        Promise.allSettled([
          createRoutine({
            name: "Daily Briefing",
            trigger: "daily_summary",
            action: "open_inbox",
            mission_id: null,
            session_id: null,
          }).then((routine) => {
            setRoutineCache((prev) => {
              if (prev.some((item) => item.name === routine.name)) return prev;
              return [routine, ...prev];
            });
          }),
          updateAutonomySettings("nudge").then(() => {
            setAutonomyMode("nudge");
          }),
          saveWorkspace({
            name: "Morning Review",
            created_at: new Date().toISOString(),
            panels: { sidebar: true, terminal: false, editor: false, git: true },
            layout: { sidebar_width: 320, terminal_height: 0, side_panel_width: 480 },
            open_files: [],
            active_file: null,
            terminal_tabs: [],
            session_id: activeSessionId,
            git_branch: activeProject?.git_branch ?? null,
            is_template: false,
            recipe_description: "Start the day with missions, inbox, and git context ready.",
            recipe_next_action: "Review the assistant summary, clear blockers, then choose the next mission.",
            is_recipe: true,
          }).then(() => {
            fetchWorkspaces().then((resp) => setWorkspaceCache(resp.workspaces)).catch(() => {});
          }),
        ]).finally(() => {
          addToast("Daily Copilot preset enabled", "success");
        });
        break;
    }
  }, [assistantPulse, activeSessionId, activeProject]);

  // ── Restore session from URL on first app state load ──────────────
  const urlRestoredRef = useRef(false);
  useEffect(() => {
    if (!appState || urlRestoredRef.current) return;
    urlRestoredRef.current = true;

    const urlSid = initialUrlState.sessionId;
    const urlProjIdx = initialUrlState.projectIdx;
    if (!urlSid) return; // nothing to restore

    // Check if the session the server already has active matches the URL
    const currentSid = appState.projects[appState.active_project]?.active_session;
    if (currentSid === urlSid) return; // already there

    // Find which project owns this session
    let targetProject = urlProjIdx;
    if (targetProject === null) {
      for (let i = 0; i < appState.projects.length; i++) {
        if (appState.projects[i].sessions.some((s) => s.id === urlSid)) {
          targetProject = i;
          break;
        }
      }
    }
    if (targetProject === null) return; // session not found

    // Fire the API calls to switch project + select session
    (async () => {
      try {
        if (targetProject !== appState.active_project) {
          await switchProject(targetProject!);
        }
        await selectSession(targetProject!, urlSid);
        refreshState();
      } catch {
        // Silently ignore — URL might have a stale session
      }
    })();
  }, [appState, initialUrlState, refreshState]);

  // ── Sync URL ← app state (and handle popstate) ───────────────────
  const handlePopState = useCallback(
    (state: UrlState) => {
      // Restore panels
      setSidebarOpen(state.panels.sidebar);
      setTerminalOpen(state.panels.terminal);
      setNeovimOpen(state.panels.editor);
      setGitOpen(state.panels.git);

      // Restore session
      if (state.sessionId && appState) {
        const currentSid =
          appState.projects[appState.active_project]?.active_session;
        if (currentSid !== state.sessionId) {
          const projIdx = state.projectIdx ?? appState.active_project;
          (async () => {
            try {
              if (projIdx !== appState.active_project) {
                await switchProject(projIdx);
              }
              await selectSession(projIdx, state.sessionId!);
              refreshState();
            } catch {
              // ignore
            }
          })();
        }
      }
    },
    [appState, refreshState]
  );

  useUrlState({
    sessionId: activeSessionId,
    projectIdx: appState?.active_project ?? 0,
    panels: {
      sidebar: sidebarOpen,
      terminal: terminalOpen,
      editor: neovimOpen,
      git: gitOpen,
    },
    onPopState: handlePopState,
  });

  // Derive current model display name from selectedModel or latest assistant message
  const currentModel = useMemo(() => {
    if (selectedModel) return selectedModel.modelID;
    for (let i = messages.length - 1; i >= 0; i--) {
      const msg = messages[i];
      if (msg.info.role === "assistant") {
        if (msg.info.modelID) return msg.info.modelID;
        if (msg.info.model) {
          if (typeof msg.info.model === "string") return msg.info.model;
          return msg.info.model.modelID || null;
        }
      }
    }
    return null;
  }, [selectedModel, messages]);

  // Derive default model for new session display (from provider defaults)
  const defaultModelDisplay = useMemo(() => {
    if (currentModel) return currentModel;
    if (selectedModel) return selectedModel.modelID;
    // Use provider defaults: find the first default model
    const defaultEntries = Object.entries(providers.defaults);
    if (defaultEntries.length > 0) {
      // Return the model ID from the first provider default
      return defaultEntries[0][1]; // [providerID, modelID]
    }
    return null;
  }, [currentModel, selectedModel, providers.defaults]);

  // Derive context limit for the current model from providers
  const currentModelContextLimit = useMemo(() => {
    const modelId = currentModel || defaultModelDisplay;
    if (!modelId || !providers.all.length) return null;
    for (const provider of providers.all) {
      for (const [, model] of Object.entries(provider.models)) {
        if (model.id === modelId && model.limit?.context) {
          return model.limit.context;
        }
      }
    }
    return null;
  }, [currentModel, defaultModelDisplay, providers.all]);

  // ── Stable toggle callbacks ────────────────────────────
  const toggleSidebar = useCallback(() => setSidebarOpen((v) => !v), []);
  const toggleTerminal = useCallback(() => setTerminalOpen((v) => !v), []);
  const toggleNeovim = useCallback(() => setNeovimOpen((v) => !v), []);
  const toggleGit = useCallback(() => setGitOpen((v) => !v), []);
  const toggleMobileSidebar = useCallback(() => setMobileSidebarOpen((v) => !v), []);

  // ── Mobile input autohide handlers ──
  const handleScrollDirection = useCallback(
    (direction: "up" | "down") => {
      // Only active on mobile (CSS handles the viewport check via .mobile-input-wrapper,
      // but we also gate state changes to avoid unnecessary re-renders on desktop).
      if (typeof window !== "undefined" && window.innerWidth >= 768) return;

      // Only hide on scroll-up; reveal is done exclusively via the compose button
      if (direction !== "up") return;

      // Debounce 150ms to absorb keyboard open/close scroll noise
      if (mobileInputDebounceRef.current) {
        clearTimeout(mobileInputDebounceRef.current);
      }
      mobileInputDebounceRef.current = window.setTimeout(() => {
        // Guard: don't hide when textarea has content
        if (hasPromptContentRef.current) return;
        setMobileInputHidden(true);
        mobileInputDebounceRef.current = 0;
      }, 150);
    },
    []
  );

  const handlePromptContentChange = useCallback(
    (hasContent: boolean) => {
      hasPromptContentRef.current = hasContent;
    },
    []
  );

  const handleComposeButtonTap = useCallback(() => {
    setMobileInputHidden(false);
    // Focus textarea after a tick so the wrapper has time to expand
    requestAnimationFrame(() => {
      const textarea = document.querySelector<HTMLTextAreaElement>(".prompt-textarea");
      textarea?.focus();
    });
  }, []);

  // ── Panel focus handlers (for unfocused dimming) ───────
  const focusSidebar = useCallback(() => setFocusedPanel("sidebar"), []);
  const focusChat = useCallback(() => setFocusedPanel("chat"), []);
  const focusSide = useCallback(() => setFocusedPanel("side"), []);

  const closeMobileSidebar = useCallback(() => setMobileSidebarOpen(false), []);
  /** Toggle a full-screen mobile panel. If already active, go back to OpenCode (chat). */
  const toggleMobilePanel = useCallback(
    (panel: "opencode" | "git" | "editor" | "terminal") => {
      setActiveMobilePanel((prev) => (prev === panel ? null : panel));
      // Mark panel as mounted so it stays alive when hidden
      if (panel !== "opencode") {
        setMobilePanelsMounted((prev) => {
          if (prev.has(panel)) return prev;
          const next = new Set(prev);
          next.add(panel);
          return next;
        });
      }
    },
    []
  );
  const closeTerminal = useCallback(() => setTerminalOpen(false), []);
  const closeNeovim = useCallback(() => setNeovimOpen(false), []);
  const closeGit = useCallback(() => setGitOpen(false), []);

  const openCommandPalette = useCallback(() => setCommandPaletteOpen(true), []);
  const closeCommandPalette = useCallback(() => setCommandPaletteOpen(false), []);
  const openModelPicker = useCallback(() => setModelPickerOpen(true), []);
  const closeModelPicker = useCallback(() => setModelPickerOpen(false), []);
  const openAgentPicker = useCallback(() => setAgentPickerOpen(true), []);
  const closeAgentPicker = useCallback(() => setAgentPickerOpen(false), []);
  const closeThemeSelector = useCallback(() => setThemeSelectorOpen(false), []);
  const openCheatsheet = useCallback(() => setCheatsheetOpen(true), []);

  /** Surface panel errors as toast notifications */
  const handlePanelError = useCallback(
    (msg: string) => addToast(msg, "error"),
    [addToast],
  );
  const closeCheatsheet = useCallback(() => setCheatsheetOpen(false), []);
  const openTodoPanel = useCallback(() => setTodoPanelOpen(true), []);
  const closeTodoPanel = useCallback(() => setTodoPanelOpen(false), []);
  const openSessionSelector = useCallback(() => setSessionSelectorOpen(true), []);
  const closeSessionSelector = useCallback(() => setSessionSelectorOpen(false), []);
  const openContextInput = useCallback(() => setContextInputOpen(true), []);
  const closeContextInput = useCallback(() => setContextInputOpen(false), []);
  const openSettings = useCallback(() => setSettingsOpen(true), []);
  const closeSettings = useCallback(() => setSettingsOpen(false), []);
  const openWatcher = useCallback(() => setWatcherOpen(true), []);
  const closeWatcher = useCallback(() => setWatcherOpen(false), []);
  const openContextWindow = useCallback(() => setContextWindowOpen(true), []);
  const closeContextWindow = useCallback(() => setContextWindowOpen(false), []);
  const openDiffReview = useCallback(() => setDiffReviewOpen(true), []);
  const closeDiffReview = useCallback(() => setDiffReviewOpen(false), []);
  const openSearchBar = useCallback(() => setSearchBarOpen(true), []);
  const closeSearchBar = useCallback(() => {
    setSearchBarOpen(false);
    setSearchMatchIds(new Set());
    setActiveSearchMatchId(null);
  }, []);
  const openCrossSearch = useCallback(() => setCrossSearchOpen(true), []);
  const closeCrossSearch = useCallback(() => setCrossSearchOpen(false), []);
  const openSplitView = useCallback(() => setSplitViewOpen(true), []);
  const closeSplitView = useCallback(() => { setSplitViewOpen(false); setSplitViewSecondaryId(null); }, []);
  const openSessionGraph = useCallback(() => setSessionGraphOpen(true), []);
  const closeSessionGraph = useCallback(() => setSessionGraphOpen(false), []);
  const openSessionDashboard = useCallback(() => setSessionDashboardOpen(true), []);
  const closeSessionDashboard = useCallback(() => setSessionDashboardOpen(false), []);
  const openActivityFeed = useCallback(() => setActivityFeedOpen(true), []);
  const closeActivityFeed = useCallback(() => setActivityFeedOpen(false), []);
  const openNotificationPrefs = useCallback(() => setNotificationPrefsOpen(true), []);
  const closeNotificationPrefs = useCallback(() => setNotificationPrefsOpen(false), []);
  const openAssistantCenter = useCallback(() => setAssistantCenterOpen(true), []);
  const closeAssistantCenter = useCallback(() => setAssistantCenterOpen(false), []);
  const openInbox = useCallback(() => setInboxOpen(true), []);
  const closeInbox = useCallback(() => setInboxOpen(false), []);
  const openMemory = useCallback(() => setMemoryOpen(true), []);
  const closeMemory = useCallback(() => setMemoryOpen(false), []);
  const openAutonomy = useCallback(() => setAutonomyOpen(true), []);
  const closeAutonomy = useCallback(() => setAutonomyOpen(false), []);
  const openRoutines = useCallback(() => setRoutinesOpen(true), []);
  const closeRoutines = useCallback(() => setRoutinesOpen(false), []);
  const openDelegation = useCallback(() => setDelegationOpen(true), []);
  const closeDelegation = useCallback(() => setDelegationOpen(false), []);
  const openMissions = useCallback(() => setMissionsOpen(true), []);
  const closeMissions = useCallback(() => setMissionsOpen(false), []);
  const openWorkspaceManager = useCallback(() => setWorkspaceManagerOpen(true), []);
  const closeWorkspaceManager = useCallback(() => setWorkspaceManagerOpen(false), []);

  /** Build a WorkspaceSnapshot from the current UI state. */
  const buildCurrentSnapshot = useCallback((): WorkspaceSnapshot => {
    return {
      name: "",
      created_at: new Date().toISOString(),
      panels: {
        sidebar: sidebarOpen,
        terminal: terminalOpen,
        editor: neovimOpen,
        git: gitOpen,
      },
      layout: {
        sidebar_width: 0,
        terminal_height: 0,
        side_panel_width: 0,
      },
      open_files: [],
      active_file: null,
      terminal_tabs: [],
      session_id: activeSessionId,
      git_branch: null,
      is_template: false,
    };
  }, [sidebarOpen, terminalOpen, neovimOpen, gitOpen, activeSessionId]);

  /** Handle search match updates from SearchBar */
  const handleSearchMatchesChanged = useCallback(
    (matchIds: Set<string>, activeId: string | null) => {
      setSearchMatchIds(matchIds);
      setActiveSearchMatchId(activeId);
    },
    []
  );

  const openModelPickerFromPalette = useCallback(() => {
    setCommandPaletteOpen(false);
    setModelPickerOpen(true);
  }, []);

  const handleThemeApplied = useCallback(
    (colors: ThemeColors) => {
      applyThemeToCss(colors);
      addToast("Theme applied", "success");
    },
    [addToast]
  );

  /** Handle context submission — sends context as a message to the session */
  const handleContextSubmit = useCallback(
    async (text: string) => {
      if (!activeSessionId) return;
      try {
        await sendMessage(
          activeSessionId,
          injectMemoryGuidance(text, activeMemoryItems),
          selectedModel ?? undefined
        );
        addToast("Context sent", "success");
      } catch {
        addToast("Failed to send context", "error");
      }
    },
    [activeSessionId, selectedModel, addToast, activeMemoryItems]
  );

  // ── Handlers ──────────────────────────────────────────

  const handleSend = useCallback(
    async (text: string, images?: ImageAttachment[]) => {
      if (!activeSessionId || sending) return;
      setSending(true);
      // Show the user's message immediately (optimistic UI)
      addOptimisticMessage(text);
      // On mobile, hide the input area after sending — user reveals via compose button
      if (typeof window !== "undefined" && window.innerWidth < 768) {
        setMobileInputHidden(true);
      }
      try {
        await sendMessage(
          activeSessionId,
          injectMemoryGuidance(text, activeMemoryItems),
          selectedModel ?? undefined,
          images,
          selectedAgent || undefined
        );
        // SSE will deliver the real server message — no REST fetch needed.
      } catch (e) {
        addToast("Failed to send message", "error");
      } finally {
        setSending(false);
      }
    },
    [activeSessionId, sending, selectedModel, selectedAgent, addToast, addOptimisticMessage, activeMemoryItems]
  );

  const handleAbort = useCallback(async () => {
    if (!activeSessionId) return;
    try {
      await abortSession(activeSessionId);
      addToast("Session aborted", "info");
    } catch {
      addToast("Failed to abort session", "error");
    }
  }, [activeSessionId, addToast]);

  const handleAgentChange = useCallback(
    async (agentId: string) => {
      setSelectedAgent(agentId);
      // Execute the /agent command on the server to persist the switch
      if (activeSessionId) {
        try {
          await executeCommand(activeSessionId, "agent", agentId);
        } catch {
          // Agent switch is best-effort; the local state is already updated
        }
      }
      addToast(`Agent switched to ${agentId}`, "success");
    },
    [activeSessionId, addToast]
  );

  const handleCommand = useCallback(
    async (command: string, args?: string) => {
      if (command === "new") {
        if (!appState) return;
        try {
          const resp = await newSession(appState.active_project);
          refreshState();
          setSelectedModel(null);
          setSelectedAgent("");
          addToast("New session created", "success");
        } catch {
          addToast("Failed to create session", "error");
        }
        return;
      }

      if (command === "terminal") {
        setTerminalOpen((v) => !v);
        return;
      }

      if (command === "models") {
        setModelPickerOpen(true);
        return;
      }

      if (command === "theme") {
        setThemeSelectorOpen(true);
        return;
      }

      if (command === "neovim" || command === "nvim") {
        setNeovimOpen((v) => !v);
        return;
      }

      if (command === "git") {
        setGitOpen((v) => !v);
        return;
      }

      if (command === "keys" || command === "keybindings") {
        setCheatsheetOpen(true);
        return;
      }

      if (command === "todos") {
        setTodoPanelOpen(true);
        return;
      }

      if (command === "sessions") {
        setSessionSelectorOpen(true);
        return;
      }

      if (command === "context") {
        setContextInputOpen(true);
        return;
      }

      if (command === "settings") {
        setSettingsOpen(true);
        return;
      }

      if (command === "watcher") {
        setWatcherOpen(true);
        return;
      }

      if (command === "context-window") {
        setContextWindowOpen(true);
        return;
      }

      if (command === "diff-review") {
        setDiffReviewOpen(true);
        return;
      }

      if (command === "search") {
        setSearchBarOpen(true);
        return;
      }

      if (command === "cross-search") {
        setCrossSearchOpen(true);
        return;
      }

      if (command === "split-view") {
        setSplitViewOpen((v) => !v);
        return;
      }

      if (command === "session-graph") {
        setSessionGraphOpen(true);
        return;
      }

      if (command === "session-dashboard") {
        setSessionDashboardOpen(true);
        return;
      }

      if (command === "activity-feed") {
        setActivityFeedOpen(true);
        return;
      }

      if (command === "notification-prefs") {
        setNotificationPrefsOpen(true);
        return;
      }

      if (command === "assistant-center") {
        setAssistantCenterOpen(true);
        return;
      }

      if (command === "inbox") {
        setInboxOpen(true);
        return;
      }

      if (command === "memory") {
        setMemoryOpen(true);
        return;
      }

      if (command === "autonomy") {
        setAutonomyOpen(true);
        return;
      }

      if (command === "routines") {
        setRoutinesOpen(true);
        return;
      }

      if (command === "delegation") {
        setDelegationOpen(true);
        return;
      }

      if (command === "missions") {
        setMissionsOpen(true);
        return;
      }

      if (command === "workspaces") {
        setWorkspaceManagerOpen(true);
        return;
      }

      if (command === "model" && !args) {
        setModelPickerOpen(true);
        return;
      }

      // "/model <name>" – open the model picker pre-filtered instead of calling
      // the broken command endpoint. The user can confirm selection there.
      if (command === "model" && args) {
        setModelPickerOpen(true);
        return;
      }

      // /agent — open agent picker modal
      if (command === "agent") {
        setAgentPickerOpen(true);
        return;
      }

      if (!activeSessionId) return;
      try {
        await executeCommand(activeSessionId, command, args);
        refreshState();
      } catch {
        addToast(`Command /${command} failed`, "error");
      }
    },
    [activeSessionId, appState, refreshState, addToast, handleAgentChange]
  );

  const handlePermissionReply = useCallback(
    async (requestId: string, reply: "once" | "always" | "reject") => {
      try {
        await replyPermission(requestId, reply);
        clearPermission(requestId);
      } catch {
        addToast("Failed to send permission reply", "error");
      }
    },
    [clearPermission, addToast]
  );

  const handleQuestionReply = useCallback(
    async (requestId: string, answers: string[][]) => {
      try {
        await replyQuestion(requestId, answers);
        clearQuestion(requestId);
      } catch {
        addToast("Failed to send answer", "error");
      }
    },
    [clearQuestion, addToast]
  );

  const handleSelectSession = useCallback(
    async (sessionId: string, projectIdx: number) => {
      if (!appState) return;
      try {
        // Switch project first if selecting from a different project
        if (projectIdx !== appState.active_project) {
          await switchProject(projectIdx);
        }
        await selectSession(projectIdx, sessionId);
        refreshState();
        setMobileSidebarOpen(false);
        setSelectedModel(null);
        setSelectedAgent("");
      } catch {
        addToast("Failed to switch session", "error");
      }
    },
    [appState, refreshState, addToast]
  );

  /** Adapter for components that pass (projectIndex, sessionId) instead of (sessionId, projectIdx) */
  const handleSelectSessionByProject = useCallback(
    (projectIndex: number, sessionId: string) => handleSelectSession(sessionId, projectIndex),
    [handleSelectSession]
  );

  /** Restore a workspace snapshot — apply panel visibility and session. */
  const handleRestoreWorkspace = useCallback(
    (ws: WorkspaceSnapshot) => {
      // Apply panel visibility
      if (ws.panels.sidebar !== sidebarOpen) toggleSidebar();
      if (ws.panels.terminal !== terminalOpen) toggleTerminal();
      if (ws.panels.editor !== neovimOpen) toggleNeovim();
      if (ws.panels.git !== gitOpen) toggleGit();

      // Switch to the saved session if available
      if (ws.session_id && ws.session_id !== activeSessionId) {
        handleSelectSession(ws.session_id, appState?.active_project ?? 0);
      }

      setActiveWorkspaceName(ws.name);
    },
    [
      sidebarOpen,
      terminalOpen,
      neovimOpen,
      gitOpen,
      activeSessionId,
      appState,
      toggleSidebar,
      toggleTerminal,
      toggleNeovim,
      toggleGit,
      handleSelectSession,
    ]
  );

  const handleNewSession = useCallback(async () => {
    if (!appState) return;
    try {
      const resp = await newSession(appState.active_project);
      // The backend creates the session synchronously and returns its ID.
      // refreshState() will pick up the new active_session from web_state.
      refreshState();
      setSelectedModel(null);
      setSelectedAgent("");
      addToast("New session created", "success");
    } catch {
      addToast("Failed to create session", "error");
    }
  }, [appState, refreshState, addToast]);

  const handleSwitchProject = useCallback(
    async (index: number) => {
      try {
        await switchProject(index);
        refreshState();
        setSelectedModel(null);
        setSelectedAgent("");
      } catch {
        addToast("Failed to switch project", "error");
      }
    },
    [refreshState, addToast]
  );

  const handleModelSelected = useCallback((modelId: string, providerId: string) => {
    setSelectedModel({ providerID: providerId, modelID: modelId });
    addToast(`Model switched to ${modelId}`, "success");
  }, [addToast]);

  // ── Presence registration + heartbeat ──────────────────
  useEffect(() => {
    const clientId = getClientId();
    const interfaceType = "web";

    // Initial registration
    registerPresence(clientId, interfaceType, activeSessionId ?? undefined).catch(() => {});

    // Heartbeat every 30 seconds
    const interval = setInterval(() => {
      registerPresence(clientId, interfaceType, activeSessionId ?? undefined).catch(() => {});
    }, 30000);

    // Deregister on unmount
    return () => {
      clearInterval(interval);
      deregisterPresence(clientId).catch(() => {});
    };
  }, [activeSessionId]);

  // ── Browser notifications for session events ───────────
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.hidden) {
        lastVisibleAtRef.current = Date.now();
        return;
      }

      const awayMs = Date.now() - lastVisibleAtRef.current;
      if (awayMs < 5 * 60 * 1000) return;

      const briefing = buildResumeBriefing({
        activeSessionId,
        missions: missionCache,
        permissions,
        questions,
        activityEvents: sse.liveActivityEvents,
        signals: assistantSignals,
      });

      if (briefing && autonomyMode !== "observe") {
        setResumeBriefing(briefing);
        setAssistantCenterOpen(true);
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => document.removeEventListener("visibilitychange", handleVisibilityChange);
  }, [activeSessionId, missionCache, permissions, questions, sse.liveActivityEvents, assistantSignals, autonomyMode]);

  useEffect(() => {
    fetchPersonalMemory()
      .then((resp) => setPersonalMemory(resp.memory ?? []))
      .catch(() => {});
  }, [memoryOpen]);

  useEffect(() => {
    fetchAutonomySettings()
      .then((settings) => setAutonomyMode(settings.mode ?? "observe"))
      .catch(() => {});
  }, [autonomyOpen]);

  useEffect(() => {
    fetchRoutines()
      .then((resp) => {
        setRoutineCache(resp.routines ?? []);
        setRoutineRunCache(resp.runs ?? []);
      })
      .catch(() => {});
  }, [routinesOpen, assistantCenterOpen]);

  useEffect(() => {
    fetchMissions()
      .then((resp) => setMissionCache(resp.missions ?? []))
      .catch(() => {});
  }, [missionsOpen, routinesOpen, assistantCenterOpen]);

  useEffect(() => {
    fetchDelegatedWork()
      .then((resp) => setDelegatedWorkCache(resp.items ?? []))
      .catch(() => {});
  }, [delegationOpen, assistantCenterOpen]);

  useEffect(() => {
    fetchWorkspaces()
      .then((resp) => setWorkspaceCache(resp.workspaces ?? []))
      .catch(() => {});
  }, [workspaceManagerOpen, assistantCenterOpen]);

  useEffect(() => {
    if (autonomyMode !== "continue" && autonomyMode !== "autonomous") return;
    if (sessionStatus !== "idle" || !activeSessionId) return;

    const routine = routineCache.find(
      (item) => item.trigger === "on_session_idle" && item.session_id === activeSessionId
    );
    if (!routine) return;
    if (executedAutoRoutineIds.includes(`${routine.id}:${activeSessionId}`)) return;

    runRoutine(routine.id, {
      summary: buildRoutineSummary(routine, { activeSessionId }),
    }).catch(() => {});

    if (routine.action === "open_inbox") {
      setInboxOpen(true);
    } else if (routine.action === "open_activity_feed") {
      setActivityFeedOpen(true);
    } else if (routine.action === "review_mission" && routine.mission_id) {
      setMissionsOpen(true);
    }

    setAssistantSignals((prev) => {
      const id = `routine-auto:${routine.id}:${activeSessionId}`;
      if (prev.some((signal) => signal.id === id)) return prev;
      return [
        {
          id,
          kind: "watcher_trigger" as const,
          title: `Routine: ${routine.name}`,
          body: `Auto-run ready for ${routine.action}`,
          createdAt: Date.now(),
          sessionId: activeSessionId,
        },
        ...prev,
      ].slice(0, 25);
    });

    setExecutedAutoRoutineIds((prev) => [...prev, `${routine.id}:${activeSessionId}`]);
  }, [autonomyMode, sessionStatus, activeSessionId, routineCache, executedAutoRoutineIds]);

  useEffect(() => {
    if (sessionStatus === "busy") {
      setExecutedAutoRoutineIds([]);
    }
  }, [sessionStatus]);

  useEffect(() => {
    if (autonomyMode !== "continue" && autonomyMode !== "autonomous") return;

    const routine = routineCache.find((item) => item.trigger === "daily_summary");
    if (!routine) return;

    const todayKey = new Date().toISOString().slice(0, 10);
    const ranToday = routineRunCache.some(
      (run) => run.routine_id === routine.id && run.created_at.slice(0, 10) === todayKey
    );
    if (ranToday) return;

    const summary = buildDailySummary({
      routine,
      missions: missionCache,
      permissions,
      questions,
      signals: assistantSignals,
    });

    setLatestDailySummary(summary);
    runRoutine(routine.id, { summary })
      .then((run) => {
        setRoutineRunCache((prev) => [run, ...prev]);
      })
      .catch(() => {});

    setAssistantSignals((prev) => {
      const id = `routine-daily:${routine.id}:${todayKey}`;
      if (prev.some((signal) => signal.id === id)) return prev;
      return [
        {
          id,
          kind: "session_complete" as const,
          title: `Daily summary: ${routine.name}`,
          body: summary,
          createdAt: Date.now(),
          sessionId: activeSessionId,
        },
        ...prev,
      ].slice(0, 25);
    });
  }, [autonomyMode, routineCache, routineRunCache, activeSessionId, missionCache, permissions, questions, assistantSignals]);

  useEffect(() => {
    const dailyRoutineIds = new Set(
      routineCache.filter((item) => item.trigger === "daily_summary").map((item) => item.id)
    );
    const latest = routineRunCache.find((run) => dailyRoutineIds.has(run.routine_id));
    if (latest?.summary) {
      setLatestDailySummary(latest.summary);
    }
  }, [routineCache, routineRunCache]);

  useEffect(() => {
    const prefs = loadNotificationPrefs();
    if (!prefs.enabled) return;

    // Session completed (status changed to idle)
    if (sessionStatus === "idle" && prefs.session_complete && autonomyMode !== "observe") {
      setAssistantSignals((prev) => {
        const next: AssistantSignal = {
          id: `session-complete:${activeSessionId ?? "none"}:${Date.now()}`,
          kind: "session_complete",
          title: "Session complete",
          body: "AI session has finished processing",
          createdAt: Date.now(),
          sessionId: activeSessionId,
        };
        return [next, ...prev].slice(0, 25);
      });
      showNotification(
        "session_complete" as NotifyEventKind,
        "Session Complete",
        "AI session has finished processing",
        prefs,
        () => window.focus()
      );
    }
  }, [sessionStatus, activeSessionId, autonomyMode]);

  useEffect(() => {
    if (!watcherStatus || watcherStatus.action !== "triggered" || autonomyMode === "observe") return;
    setAssistantSignals((prev) => {
      const id = `watcher-trigger:${watcherStatus.session_id}:${Date.now()}`;
      const next: AssistantSignal = {
        id,
        kind: "watcher_trigger",
        title: "Watcher triggered",
        body: "A watched session auto-continued and may need review.",
        createdAt: Date.now(),
        sessionId: watcherStatus.session_id,
      };
      return [next, ...prev].slice(0, 25);
    });
  }, [watcherStatus, autonomyMode]);

  // ── Keyboard shortcuts ────────────────────────────────

  useKeyboard([
    {
      key: "p",
      meta: true,
      shift: true,
      handler: openCommandPalette,
      description: "Command Palette",
    },
    {
      key: "'",
      meta: true,
      handler: openModelPicker,
      description: "Model Picker",
    },
    {
      key: "b",
      meta: true,
      handler: toggleSidebar,
      description: "Toggle Sidebar",
    },
    {
      key: "`",
      meta: true,
      handler: toggleTerminal,
      description: "Toggle Terminal",
    },
    {
      key: "n",
      meta: true,
      shift: true,
      handler: handleNewSession,
      description: "New Session",
    },
    {
      key: "e",
      meta: true,
      shift: true,
      handler: toggleNeovim,
      description: "Toggle Editor",
    },
    {
      key: "g",
      meta: true,
      shift: true,
      handler: toggleGit,
      description: "Toggle Git",
    },
    {
      key: "t",
      meta: true,
      shift: true,
      handler: openTodoPanel,
      description: "Todo Panel",
    },
    {
      key: "s",
      meta: true,
      shift: true,
      handler: openSessionSelector,
      description: "Session Selector",
    },
    {
      key: "k",
      meta: true,
      shift: true,
      handler: openContextInput,
      description: "Context Input",
    },
    {
      key: ",",
      meta: true,
      handler: openSettings,
      description: "Settings",
    },
    {
      key: "w",
      meta: true,
      shift: true,
      handler: openWatcher,
      description: "Session Watcher",
    },
    {
      key: "c",
      meta: true,
      shift: true,
      handler: openContextWindow,
      description: "Context Window",
    },
    {
      key: "d",
      meta: true,
      shift: true,
      handler: openDiffReview,
      description: "Diff Review",
    },
    {
      key: "f",
      meta: true,
      handler: openSearchBar,
      description: "Search in Conversation",
    },
    {
      key: "f",
      meta: true,
      shift: true,
      handler: openCrossSearch,
      description: "Search All Sessions",
    },
    {
      key: "\\",
      meta: true,
      handler: () => setSplitViewOpen((v) => !v),
      description: "Toggle Split View",
    },
    {
      key: "h",
      meta: true,
      shift: true,
      handler: openSessionGraph,
      description: "Session Graph",
    },
    {
      key: "o",
      meta: true,
      shift: true,
      handler: openSessionDashboard,
      description: "Session Dashboard",
    },
    {
      key: "a",
      meta: true,
      shift: true,
      handler: openActivityFeed,
      description: "Activity Feed",
    },
    {
      key: "i",
      meta: true,
      shift: true,
      handler: openNotificationPrefs,
      description: "Notification Preferences",
    },
    {
      key: ".",
      meta: true,
      shift: true,
      handler: openAssistantCenter,
      description: "Assistant Center",
    },
    {
      key: "u",
      meta: true,
      shift: true,
      handler: openInbox,
      description: "Assistant Inbox",
    },
    {
      key: "b",
      meta: true,
      shift: true,
      handler: openDelegation,
      description: "Delegation Board",
    },
    {
      key: "r",
      meta: true,
      shift: true,
      handler: openRoutines,
      description: "Routines",
    },
    {
      key: "j",
      meta: true,
      shift: true,
      handler: openAutonomy,
      description: "Autonomy",
    },
    {
      key: "y",
      meta: true,
      shift: true,
      handler: openMemory,
      description: "Personal Memory",
    },
    {
      key: "m",
      meta: true,
      shift: true,
      handler: openMissions,
      description: "Missions",
    },
    {
      key: "l",
      meta: true,
      shift: true,
      handler: openWorkspaceManager,
      description: "Workspace Manager",
    },
    {
      key: "?",
      shift: true,
      handler: () => setCheatsheetOpen((v) => !v),
      description: "Keybinding Cheatsheet",
    },
    {
      key: "Escape",
      handler: () => {
        if (commandPaletteOpen) setCommandPaletteOpen(false);
        else if (modelPickerOpen) setModelPickerOpen(false);
        else if (agentPickerOpen) setAgentPickerOpen(false);
        else if (themeSelectorOpen) setThemeSelectorOpen(false);
        else if (cheatsheetOpen) setCheatsheetOpen(false);
        else if (todoPanelOpen) setTodoPanelOpen(false);
        else if (sessionSelectorOpen) setSessionSelectorOpen(false);
        else if (contextInputOpen) setContextInputOpen(false);
        else if (settingsOpen) setSettingsOpen(false);
        else if (watcherOpen) setWatcherOpen(false);
        else if (contextWindowOpen) setContextWindowOpen(false);
        else if (diffReviewOpen) setDiffReviewOpen(false);
        else if (searchBarOpen) { setSearchBarOpen(false); setSearchMatchIds(new Set()); setActiveSearchMatchId(null); }
        else if (crossSearchOpen) setCrossSearchOpen(false);
        else if (activityFeedOpen) setActivityFeedOpen(false);
        else if (notificationPrefsOpen) setNotificationPrefsOpen(false);
        else if (assistantCenterOpen) setAssistantCenterOpen(false);
        else if (inboxOpen) setInboxOpen(false);
        else if (memoryOpen) setMemoryOpen(false);
        else if (autonomyOpen) setAutonomyOpen(false);
        else if (routinesOpen) setRoutinesOpen(false);
        else if (delegationOpen) setDelegationOpen(false);
        else if (missionsOpen) setMissionsOpen(false);
        else if (workspaceManagerOpen) setWorkspaceManagerOpen(false);
      },
    },
  ]);

  // ── Render ────────────────────────────────────────────

  if (!appState) {
    return (
      <div className="chat-loading">
        <div className="chat-loading-spinner" />
        <span>Connecting to opman...</span>
      </div>
    );
  }

  const hasSidePanel = neovimOpen || gitOpen;

  return (
    <div className="chat-layout">
      {/* Mobile sidebar overlay */}
      {mobileSidebarOpen && (
        <div
          className="sidebar-overlay visible"
          onClick={closeMobileSidebar}
        />
      )}

      {/* Content area: sidebar + main + side panel */}
      <div className="chat-content">
        {/* Sidebar */}
        {sidebarOpen && (
          <>
            <div
              style={{ width: sidebarResize.size, flexShrink: 0 }}
              className={focusedPanel !== "sidebar" ? "panel-dimmed" : ""}
              onMouseDown={focusSidebar}
              onFocus={focusSidebar}
            >
              <ChatSidebar
                projects={appState.projects}
                activeProject={appState.active_project}
                activeSessionId={activeSessionId}
                busySessions={busySessions}
                onSelectSession={handleSelectSession}
                onNewSession={handleNewSession}
                onSwitchProject={handleSwitchProject}
                isMobileOpen={mobileSidebarOpen}
                onClose={closeMobileSidebar}
              />
            </div>
            {/* Sidebar resize handle */}
            <div {...sidebarResize.handleProps} />
          </>
        )}

        {/* Main chat area */}
        <div
          className={`chat-main${focusedPanel !== "chat" ? " panel-dimmed" : ""}`}
          onMouseDown={focusChat}
          onFocus={focusChat}
        >
          {/* Mobile floating status pill — replaces traditional header */}
          <div className="chat-mobile-header">
            <button
              className="mobile-status-pill"
              onClick={toggleMobileSidebar}
              aria-label={mobileSidebarOpen ? "Close sidebar" : "Open sessions"}
            >
              <Sparkles size={14} className="mobile-pill-icon" />
              <span className="mobile-project-name">
                {activeProject?.name || "opman"}
              </span>
              {sessionStatus === "busy" && <span className="mobile-pill-busy" />}
            </button>
            <button
              className="mobile-cmd-btn"
              onClick={openCommandPalette}
              aria-label="Open command palette"
            >
              <Command size={14} />
            </button>
          </div>

          {/* In-session search bar */}
          {searchBarOpen && (
            <SearchBar
              messages={messages}
              onClose={closeSearchBar}
              onMatchesChanged={handleSearchMatchesChanged}
            />
          )}

          {/* Message timeline */}
          <MessageTimeline
            messages={messages}
            sessionStatus={sessionStatus}
            activeSessionId={activeSessionId}
            isLoadingMessages={isLoadingMessages}
            isLoadingOlder={isLoadingOlder}
            hasOlderMessages={hasOlderMessages}
            totalMessageCount={totalMessageCount}
            onLoadOlder={loadOlderMessages}
            appState={appState}
            defaultModel={defaultModelDisplay}
            onSendPrompt={handleSend}
            subagentMessages={subagentMessages}
            searchMatchIds={searchMatchIds}
            activeSearchMatchId={activeSearchMatchId}
            isBookmarked={isBookmarked}
            onToggleBookmark={toggleBookmark}
            onScrollDirection={handleScrollDirection}
            onOpenSession={(sessionId) => handleSelectSession(sessionId, appState?.active_project ?? 0)}
          />

          {/* Mobile input wrapper — wraps docks + prompt for autohide animation */}
          <div className={`mobile-input-wrapper${mobileInputHidden ? " mobile-input-hidden" : ""}`}>
            {/* Permission dock */}
            {allPermissions.length > 0 && (
              <PermissionDock
                permissions={allPermissions}
                activeSessionId={activeSessionId}
                onReply={handlePermissionReply}
              />
            )}

            {/* Question dock */}
            {allQuestions.length > 0 && (
              <QuestionDock
                questions={allQuestions}
                activeSessionId={activeSessionId}
                onReply={handleQuestionReply}
              />
            )}

            {/* Prompt input */}
            <PromptInput
              onSend={handleSend}
              onAbort={handleAbort}
              onCommand={handleCommand}
              onOpenModelPicker={openModelPicker}
              onOpenAgentPicker={openAgentPicker}
              isBusy={sessionStatus === "busy"}
              isSending={sending}
              disabled={!activeSessionId}
              sessionId={activeSessionId}
              currentModel={currentModel}
              currentAgent={selectedAgent}
              onAgentChange={handleAgentChange}
              activeMemoryLabels={activeMemoryItems.map((item) => item.label)}
              onOpenMemory={openMemory}
              onContentChange={handlePromptContentChange}
            />
          </div>

          {/* Terminal panel — stays mounted once opened, hidden via display:none */}
          {terminalMounted && (
            <>
              <div {...terminalResize.handleProps} style={{ ...terminalResize.handleProps.style, display: terminalOpen ? undefined : "none" }} />
              <div style={{ height: terminalResize.size, flexShrink: 0, display: terminalOpen ? undefined : "none" }}>
                <TerminalPanel
                  sessionId={activeSessionId}
                  onClose={closeTerminal}
                  visible={terminalOpen}
                  mcpAgentActive={Array.from(mcpAgentActivity.keys()).some(t => t.startsWith("web_terminal"))}
                />
              </div>
            </>
          )}
        </div>

        {/* Side panel: Editor or Git (right of main) — stays mounted once opened */}
        {(hasSidePanel || editorMounted || gitMounted) && (
          <>
            {/* Side panel resize handle */}
            <div {...sidePanelResize.handleProps} style={{ ...sidePanelResize.handleProps.style, display: hasSidePanel ? undefined : "none" }} />
            <div
              className={`side-panel${focusedPanel !== "side" ? " panel-dimmed" : ""}`}
              style={{
                width: sidePanelResize.size,
                flexShrink: 0,
                display: hasSidePanel ? undefined : "none",
              }}
              onMouseDown={focusSide}
              onFocus={focusSide}
            >
              {editorMounted && (
                <div className="side-panel-section" style={{ display: neovimOpen ? undefined : "none" }}>
                  <div className="side-panel-header">
                    <FileCode size={14} />
                    <span>Editor</span>
                    {Array.from(mcpAgentActivity.keys()).some(t => t.startsWith("web_editor")) && (
                      <span className="mcp-agent-indicator" title="AI agent active">
                        <span className="mcp-agent-dot" />
                      </span>
                    )}
                    <button
                      className="side-panel-close"
                      onClick={closeNeovim}
                      aria-label="Close editor panel"
                    >
                      <X size={14} />
                    </button>
                  </div>
                  <div className="side-panel-body">
                    <Suspense fallback={null}>
                      <CodeEditorPanel
                        focused={neovimOpen && !gitOpen}
                        openFilePath={mcpEditorOpenPath}
                        openLine={sse.mcpEditorOpenLine}
                        projectPath={activeProject?.path}
                        sessionId={activeSessionId}
                        onError={handlePanelError}
                      />
                    </Suspense>
                  </div>
                </div>
              )}
              {gitMounted && (
                <div className="side-panel-section" style={{ display: gitOpen ? undefined : "none" }}>
                  <div className="side-panel-header">
                    <GitBranch size={14} />
                    <span>Git</span>
                    <button
                      className="side-panel-close"
                      onClick={closeGit}
                      aria-label="Close git panel"
                    >
                      <X size={14} />
                    </button>
                  </div>
                  <div className="side-panel-body">
                    <Suspense fallback={null}>
                      <GitPanel
                        focused={gitOpen}
                        projectPath={activeProject?.path}
                        onError={handlePanelError}
                        onSendToAI={handleSend}
                      />
                    </Suspense>
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {/* Command palette modal */}
      {commandPaletteOpen && (
        <CommandPalette
          onClose={closeCommandPalette}
          onCommand={handleCommand}
          onNewSession={handleNewSession}
          onToggleSidebar={toggleSidebar}
          onToggleTerminal={toggleTerminal}
          onOpenModelPicker={openModelPickerFromPalette}
          onOpenCheatsheet={openCheatsheet}
          onOpenTodoPanel={openTodoPanel}
          onOpenSessionSelector={openSessionSelector}
          onOpenContextInput={openContextInput}
          onOpenSettings={openSettings}
          onOpenWatcher={openWatcher}
          onOpenContextWindow={openContextWindow}
          onOpenDiffReview={openDiffReview}
          onOpenSearch={openSearchBar}
          onOpenCrossSearch={openCrossSearch}
          onOpenSplitView={openSplitView}
          onOpenSessionGraph={openSessionGraph}
          onOpenSessionDashboard={openSessionDashboard}
          onOpenActivityFeed={openActivityFeed}
          onOpenNotificationPrefs={openNotificationPrefs}
          onOpenAssistantCenter={openAssistantCenter}
          onOpenInbox={openInbox}
          onOpenMemory={openMemory}
          onOpenAutonomy={openAutonomy}
          onOpenRoutines={openRoutines}
          onOpenDelegation={openDelegation}
          onOpenMissions={openMissions}
          onOpenWorkspaceManager={openWorkspaceManager}
          sessionId={activeSessionId}
        />
      )}

      {/* Model picker modal */}
      {modelPickerOpen && (
        <ModelPickerModal
          onClose={closeModelPicker}
          sessionId={activeSessionId}
          onModelSelected={handleModelSelected}
        />
      )}

      {/* Agent picker modal */}
      {agentPickerOpen && (
        <AgentPickerModal
          onClose={closeAgentPicker}
          currentAgent={selectedAgent}
          onAgentSelected={handleAgentChange}
        />
      )}

      {/* Theme selector modal */}
      {themeSelectorOpen && (
        <ThemeSelectorModal
          onClose={closeThemeSelector}
          onThemeApplied={handleThemeApplied}
          themeMode={themeMode}
          onThemeModeChange={setThemeMode}
        />
      )}

      {/* Cheatsheet modal */}
      {cheatsheetOpen && (
        <CheatsheetModal onClose={closeCheatsheet} />
      )}

      {/* Todo panel modal */}
      {todoPanelOpen && activeSessionId && (
        <TodoPanelModal
          onClose={closeTodoPanel}
          sessionId={activeSessionId}
        />
      )}

      {/* Session selector modal (cross-project) */}
      {sessionSelectorOpen && appState && (
        <SessionSelectorModal
          onClose={closeSessionSelector}
          projects={appState.projects}
          activeSessionId={activeSessionId}
          onSelectSession={handleSelectSession}
        />
      )}

      {/* Context input modal */}
      {contextInputOpen && (
        <ContextInputModal
          onClose={closeContextInput}
          onSubmit={handleContextSubmit}
        />
      )}

      {/* Settings modal */}
      {settingsOpen && (
        <SettingsModal
          onClose={closeSettings}
          onOpenThemeSelector={() => {
            setSettingsOpen(false);
            setThemeSelectorOpen(true);
          }}
          onOpenCheatsheet={() => {
            setSettingsOpen(false);
            setCheatsheetOpen(true);
          }}
          onOpenNotificationPrefs={() => {
            setSettingsOpen(false);
            setNotificationPrefsOpen(true);
          }}
          onOpenAssistantCenter={() => {
            setSettingsOpen(false);
            setAssistantCenterOpen(true);
          }}
          onOpenMemory={() => {
            setSettingsOpen(false);
            setMemoryOpen(true);
          }}
          onOpenAutonomy={() => {
            setSettingsOpen(false);
            setAutonomyOpen(true);
          }}
          onOpenRoutines={() => {
            setSettingsOpen(false);
            setRoutinesOpen(true);
          }}
          onOpenDelegation={() => {
            setSettingsOpen(false);
            setDelegationOpen(true);
          }}
          onOpenWorkspaceManager={() => {
            setSettingsOpen(false);
            setWorkspaceManagerOpen(true);
          }}
          onOpenInbox={() => {
            setSettingsOpen(false);
            setInboxOpen(true);
          }}
          onOpenMissions={() => {
            setSettingsOpen(false);
            setMissionsOpen(true);
          }}
          sidebarOpen={sidebarOpen}
          terminalOpen={terminalOpen}
          neovimOpen={neovimOpen}
          gitOpen={gitOpen}
          onToggleSidebar={toggleSidebar}
          onToggleTerminal={toggleTerminal}
          onToggleNeovim={toggleNeovim}
          onToggleGit={toggleGit}
        />
      )}

      {/* Watcher modal */}
      {watcherOpen && (
        <WatcherModal
          onClose={closeWatcher}
          activeSessionId={activeSessionId}
        />
      )}

      {/* Context window modal */}
      {contextWindowOpen && (
        <ContextWindowPanel
          onClose={closeContextWindow}
          sessionId={activeSessionId}
          onCompact={() => addToast("Compacting conversation...", "info")}
        />
      )}

      {/* Diff review modal */}
      {diffReviewOpen && (
        <DiffReviewPanel
          onClose={closeDiffReview}
          sessionId={activeSessionId}
          fileEditCount={fileEditCount}
        />
      )}

      {/* Cross-session search modal */}
      {crossSearchOpen && appState && (
        <CrossSessionSearchModal
          onClose={closeCrossSearch}
          projectIdx={appState.active_project}
          onNavigate={(sessionId) => handleSelectSession(sessionId, appState.active_project)}
        />
      )}

      {/* Split view */}
      {splitViewOpen && appState && activeSessionId && (
        <SplitView
          primarySessionId={activeSessionId}
          secondarySessionId={splitViewSecondaryId}
          onChangeSecondary={setSplitViewSecondaryId}
          onClose={closeSplitView}
          sessions={activeProject?.sessions ?? []}
          appState={appState}
          selectedModel={selectedModel?.modelID}
          projectIndex={appState.active_project}
        />
      )}

      {/* Session graph */}
      {sessionGraphOpen && (
        <Suspense fallback={null}>
          <SessionGraph
            onSelectSession={handleSelectSessionByProject}
            onClose={closeSessionGraph}
            activeSessionId={activeSessionId}
          />
        </Suspense>
      )}

      {/* Session dashboard */}
      {sessionDashboardOpen && (
        <Suspense fallback={null}>
          <SessionDashboard
            onSelectSession={handleSelectSessionByProject}
            onClose={closeSessionDashboard}
            activeSessionId={activeSessionId}
          />
        </Suspense>
      )}

      {/* Activity feed */}
      {activityFeedOpen && (
        <Suspense fallback={null}>
          <ActivityFeed
            sessionId={activeSessionId}
            onClose={closeActivityFeed}
            liveEvents={sse.liveActivityEvents}
          />
        </Suspense>
      )}

      {/* Notification preferences */}
      {notificationPrefsOpen && (
        <Suspense fallback={null}>
          <NotificationPrefsModal onClose={closeNotificationPrefs} />
        </Suspense>
      )}

      {assistantCenterOpen && (
        <Suspense fallback={null}>
          <AssistantCenterModal
            onClose={closeAssistantCenter}
            autonomyMode={autonomyMode}
            missions={missionCache}
            routines={routineCache}
            delegatedWork={delegatedWorkCache}
            memoryItems={activeMemoryItems}
            assistantSignals={assistantSignals}
            permissions={allPermissions}
            questions={allQuestions}
            workspaces={workspaceCache}
            resumeBriefing={resumeBriefing}
            latestDailySummary={latestDailySummary}
            onQuickSetupDailyCopilot={() => {
              Promise.allSettled([
                createRoutine({
                  name: "Daily Briefing",
                  trigger: "daily_summary",
                  action: "open_inbox",
                  mission_id: null,
                  session_id: null,
                }).then((routine) => {
                  setRoutineCache((prev) => {
                    if (prev.some((item) => item.name === routine.name)) return prev;
                    return [routine, ...prev];
                  });
                }),
                updateAutonomySettings("nudge").then(() => {
                  setAutonomyMode("nudge");
                }),
                saveWorkspace({
                  name: "Morning Review",
                  created_at: new Date().toISOString(),
                  panels: { sidebar: true, terminal: false, editor: false, git: true },
                  layout: { sidebar_width: 320, terminal_height: 0, side_panel_width: 480 },
                  open_files: [],
                  active_file: null,
                  terminal_tabs: [],
                  session_id: activeSessionId,
                  git_branch: activeProject?.git_branch ?? null,
                  is_template: false,
                  recipe_description: "Start the day with missions, inbox, and git context ready.",
                  recipe_next_action: "Review the assistant summary, clear blockers, then choose the next mission.",
                  is_recipe: true,
                }).then(() => {
                  fetchWorkspaces().then((resp) => setWorkspaceCache(resp.workspaces)).catch(() => {});
                }),
              ]).finally(() => {
                addToast("Daily Copilot preset enabled", "success");
              });
            }}
            onQuickSetupDailySummary={() => {
              createRoutine({
                name: "Daily Briefing",
                trigger: "daily_summary",
                action: "open_inbox",
                mission_id: null,
                session_id: null,
              })
                .then((routine) => {
                  setRoutineCache((prev) => [routine, ...prev]);
                  addToast("Daily briefing enabled", "success");
                })
                .catch(() => addToast("Failed to enable daily briefing", "error"));
            }}
            onQuickUpgradeAutonomy={() => {
              setAutonomyMode("nudge");
              updateAutonomySettings("nudge")
                .then(() => addToast("Autonomy set to Nudge", "success"))
                .catch(() => addToast("Failed to update autonomy", "error"));
            }}
            onOpenInbox={() => {
              setAssistantCenterOpen(false);
              setInboxOpen(true);
            }}
            onOpenMissions={() => {
              setAssistantCenterOpen(false);
              setMissionsOpen(true);
            }}
            onOpenMemory={() => {
              setAssistantCenterOpen(false);
              setMemoryOpen(true);
            }}
            onOpenAutonomy={() => {
              setAssistantCenterOpen(false);
              setAutonomyOpen(true);
            }}
            onOpenRoutines={() => {
              setAssistantCenterOpen(false);
              setRoutinesOpen(true);
            }}
            onOpenDelegation={() => {
              setAssistantCenterOpen(false);
              setDelegationOpen(true);
            }}
            onOpenWorkspaces={() => {
              setAssistantCenterOpen(false);
              setWorkspaceManagerOpen(true);
            }}
          />
        </Suspense>
      )}

      {inboxOpen && (
        <Suspense fallback={null}>
          <InboxModal
            onClose={closeInbox}
            permissions={allPermissions}
            questions={allQuestions}
            watcherStatus={watcherStatus}
            signals={assistantSignals}
            activityEvents={sse.liveActivityEvents}
            onDismissSignal={(id) =>
              setAssistantSignals((prev) => prev.filter((signal) => signal.id !== id))
            }
            onOpenMissions={() => {
              setInboxOpen(false);
              setMissionsOpen(true);
            }}
            onOpenActivityFeed={() => {
              setInboxOpen(false);
              setActivityFeedOpen(true);
            }}
            onOpenWatcher={() => {
              setInboxOpen(false);
              setWatcherOpen(true);
            }}
            onPermissionResolved={clearPermission}
            onQuestionResolved={clearQuestion}
            activeMemoryItems={personalMemory.filter((item) => {
              if (item.scope === "global") return true;
              if (item.scope === "project") return item.project_index === appState.active_project;
              return item.session_id === activeSessionId;
            })}
          />
        </Suspense>
      )}

      {missionsOpen && (
        <Suspense fallback={null}>
          <MissionsModal
            onClose={closeMissions}
            projects={appState.projects}
            activeProjectIndex={appState.active_project}
            activeSessionId={activeSessionId}
            permissions={allPermissions}
            questions={allQuestions}
            activityEvents={sse.liveActivityEvents}
            onOpenInbox={() => {
              setMissionsOpen(false);
              setInboxOpen(true);
            }}
            activeMemoryItems={activeMemoryItems}
            onOpenMissionSource={() => {
              setMissionsOpen(true);
            }}
          />
        </Suspense>
      )}

      {memoryOpen && (
        <Suspense fallback={null}>
          <MemoryModal
            onClose={closeMemory}
            projects={appState.projects}
            activeProjectIndex={appState.active_project}
            activeSessionId={activeSessionId}
          />
        </Suspense>
      )}

      {autonomyOpen && (
        <Suspense fallback={null}>
          <AutonomyModal
            onClose={closeAutonomy}
            mode={autonomyMode}
            onChange={(mode) => {
              setAutonomyMode(mode);
              updateAutonomySettings(mode).catch(() => {});
            }}
          />
        </Suspense>
      )}

      {routinesOpen && (
        <Suspense fallback={null}>
          <RoutinesModal
            onClose={closeRoutines}
            missions={missionCache}
            activeSessionId={activeSessionId}
            autonomyMode={autonomyMode}
          />
        </Suspense>
      )}

      {delegationOpen && (
        <Suspense fallback={null}>
          <DelegationBoardModal
            onClose={closeDelegation}
            missions={missionCache}
            activeSessionId={activeSessionId}
            onOpenSession={(sessionId) => handleSelectSession(sessionId, appState.active_project)}
          />
        </Suspense>
      )}

      {/* Workspace manager */}
      {workspaceManagerOpen && (
        <Suspense fallback={null}>
          <WorkspaceManagerModal
            onClose={closeWorkspaceManager}
            onRestore={handleRestoreWorkspace}
            onSaveCurrent={buildCurrentSnapshot}
            activeWorkspaceName={activeWorkspaceName}
          />
        </Suspense>
      )}

      {/* Status bar */}
      <StatusBar
        project={activeProject}
        stats={stats}
        sessionStatus={sessionStatus}
        sidebarOpen={sidebarOpen}
        terminalOpen={terminalOpen}
        neovimOpen={neovimOpen}
        gitOpen={gitOpen}
        watcherStatus={watcherStatus}
        contextLimit={currentModelContextLimit}
        presenceClients={sse.presenceClients}
        activeWorkspaceName={activeWorkspaceName}
        activeMemoryItems={personalMemory.filter((item) => {
          if (item.scope === "global") return true;
          if (item.scope === "project") return item.project_index === appState.active_project;
          return item.session_id === activeSessionId;
        })}
        autonomyMode={autonomyMode}
        assistantPulse={assistantPulse}
        onRunAssistantPulse={handleRunAssistantPulse}
        onToggleSidebar={toggleSidebar}
        onToggleTerminal={toggleTerminal}
        onToggleNeovim={toggleNeovim}
        onToggleGit={toggleGit}
        onOpenCommandPalette={openCommandPalette}
        onOpenWatcher={openWatcher}
        onOpenContextWindow={openContextWindow}
      />

      {/* Toast notifications */}
      <ToastContainer toasts={toasts} onDismiss={removeToast} />

      {/* Mobile floating dock — centered pill with ambient glow */}
      <nav className="mobile-dock" aria-label="Navigation">
        <div className="mobile-dock-inner">
          {/* Compose button — appears when mobile input is hidden */}
          {mobileInputHidden && (
            <button
              className="mobile-dock-btn mobile-compose-btn"
              onClick={handleComposeButtonTap}
              aria-label="Compose message"
            >
              <PenSquare size={20} />
            </button>
          )}
          <button
            className={`mobile-dock-btn ${activeMobilePanel === null || activeMobilePanel === "opencode" ? "active" : ""}`}
            onClick={() => toggleMobilePanel("opencode")}
            aria-label="Chat"
          >
            <MessageCircle size={20} />
          </button>
          <button
            className={`mobile-dock-btn ${activeMobilePanel === "git" ? "active" : ""}`}
            onClick={() => toggleMobilePanel("git")}
            aria-label="Git"
          >
            <GitBranch size={20} />
          </button>
          <button
            className={`mobile-dock-btn ${activeMobilePanel === "editor" ? "active" : ""}`}
            onClick={() => toggleMobilePanel("editor")}
            aria-label="Editor"
          >
            <FileCode size={20} />
          </button>
          <button
            className={`mobile-dock-btn ${activeMobilePanel === "terminal" ? "active" : ""}`}
            onClick={() => toggleMobilePanel("terminal")}
            aria-label="Terminal"
          >
            <Terminal size={20} />
          </button>
          <button
            className={`mobile-dock-btn ${assistantCenterOpen ? "active" : ""}`}
            onClick={openAssistantCenter}
            aria-label="Assistant"
          >
            <Sparkles size={20} />
          </button>
        </div>
      </nav>

      {/* Mobile panel sheets — slide-up modal sheets with glass chrome */}
      {mobilePanelsMounted.has("git") && (
        <div className={`mobile-panel-sheet ${activeMobilePanel === "git" ? "mobile-panel-active" : ""}`}>
          <div className="mobile-sheet-handle" />
          <div className="mobile-panel-header">
            <GitBranch size={15} />
            <span className="mobile-panel-title">Git</span>
            <button className="mobile-cmd-btn" onClick={openCommandPalette} aria-label="Open command palette">
              <Command size={14} />
            </button>
          </div>
          <Suspense fallback={null}>
            <GitPanel focused={activeMobilePanel === "git"} projectPath={activeProject?.path} onError={handlePanelError} onSendToAI={handleSend} />
          </Suspense>
        </div>
      )}
      {mobilePanelsMounted.has("editor") && (
        <div className={`mobile-panel-sheet ${activeMobilePanel === "editor" ? "mobile-panel-active" : ""}`}>
          <div className="mobile-sheet-handle" />
          <div className="mobile-panel-header">
            <FileCode size={15} />
            <span className="mobile-panel-title">Editor</span>
            <button className="mobile-cmd-btn" onClick={openCommandPalette} aria-label="Open command palette">
              <Command size={14} />
            </button>
          </div>
          <Suspense fallback={null}>
            <CodeEditorPanel focused={activeMobilePanel === "editor"} openFilePath={mcpEditorOpenPath} openLine={sse.mcpEditorOpenLine} projectPath={activeProject?.path} sessionId={activeSessionId} onError={handlePanelError} />
          </Suspense>
        </div>
      )}
      {mobilePanelsMounted.has("terminal") && (
        <div className={`mobile-panel-sheet ${activeMobilePanel === "terminal" ? "mobile-panel-active" : ""}`}>
          <div className="mobile-sheet-handle" />
          <div className="mobile-panel-header">
            <Terminal size={15} />
            <span className="mobile-panel-title">Terminal</span>
            <button className="mobile-cmd-btn" onClick={openCommandPalette} aria-label="Open command palette">
              <Command size={14} />
            </button>
          </div>
          <TerminalPanel
            sessionId={activeSessionId}
            onClose={() => toggleMobilePanel("terminal")}
            mcpAgentActive={Array.from(mcpAgentActivity.keys()).some(t => t.startsWith("web_terminal"))}
          />
        </div>
      )}
    </div>
  );
}

function injectMemoryGuidance(text: string, memoryItems: PersonalMemoryItem[]): string {
  if (memoryItems.length === 0) return text;

  const guidance = memoryItems
    .slice(0, 5)
    .map((item) => `- ${item.label}: ${item.content}`)
    .join("\n");

  return `[Assistant memory in effect]\n${guidance}\n\n[User request]\n${text}`;
}
