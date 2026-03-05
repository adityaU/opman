import React, { useState, useCallback, useMemo, useEffect } from "react";
import { useSSE } from "./hooks/useSSE";
import { useKeyboard } from "./hooks/useKeyboard";
import { useResizable } from "./hooks/useResizable";
import { useToast } from "./hooks/useToast";
import { ChatSidebar } from "./ChatSidebar";
import { MessageTimeline } from "./MessageTimeline";
import { PromptInput } from "./PromptInput";
import { StatusBar } from "./StatusBar";
import { CommandPalette } from "./CommandPalette";
import { ModelPickerModal } from "./ModelPickerModal";
import { PermissionDock } from "./PermissionDock";
import { QuestionDock } from "./QuestionDock";
import { TerminalPanel } from "./TerminalPanel";
import { XtermPanel } from "./XtermPanel";
import { ToastContainer } from "./ToastContainer";
import { ThemeSelectorModal } from "./ThemeSelectorModal";
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
} from "./api";
import type { ThemeColors, ModelRef } from "./api";
import { X, FileCode, GitBranch, PanelLeft, Terminal, Command } from "lucide-react";

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
]);

/** Apply theme colors as CSS custom properties on :root */
function applyTheme(colors: ThemeColors) {
  const root = document.documentElement;
  root.style.setProperty("--color-primary", colors.primary);
  root.style.setProperty("--color-secondary", colors.secondary);
  root.style.setProperty("--color-accent", colors.accent);
  root.style.setProperty("--color-bg", colors.background);
  root.style.setProperty("--color-bg-panel", colors.background_panel);
  root.style.setProperty("--color-bg-element", colors.background_element);
  root.style.setProperty("--color-text", colors.text);
  root.style.setProperty("--color-text-muted", colors.text_muted);
  root.style.setProperty("--color-border", colors.border);
  root.style.setProperty("--color-border-active", colors.border_active);
  root.style.setProperty("--color-border-subtle", colors.border_subtle);
  root.style.setProperty("--color-error", colors.error);
  root.style.setProperty("--color-warning", colors.warning);
  root.style.setProperty("--color-success", colors.success);
  root.style.setProperty("--color-info", colors.info);
}

export function ChatLayout() {
  const sse = useSSE();
  const {
    appState,
    messages,
    stats,
    busySessions,
    permissions,
    questions,
    sessionStatus,
    refreshState,
    refreshMessages,
    clearPermission,
    clearQuestion,
  } = sse;

  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [neovimOpen, setNeovimOpen] = useState(false);
  const [gitOpen, setGitOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [themeSelectorOpen, setThemeSelectorOpen] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [selectedModel, setSelectedModel] = useState<ModelRef | null>(null);
  const [sending, setSending] = useState(false);

  // Toast notifications
  const { toasts, addToast, removeToast } = useToast();

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
    fetchTheme().then((colors) => {
      if (colors) applyTheme(colors);
    });
  }, []);

  const activeProject = appState
    ? appState.projects[appState.active_project] ?? null
    : null;
  const activeSessionId = activeProject?.active_session ?? null;

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

  // ── Handlers ──────────────────────────────────────────

  const handleSend = useCallback(
    async (text: string) => {
      if (!activeSessionId || sending) return;
      setSending(true);
      try {
        await sendMessage(activeSessionId, text, selectedModel ?? undefined);
      } catch (e) {
        addToast("Failed to send message", "error");
      } finally {
        setSending(false);
      }
    },
    [activeSessionId, sending, selectedModel, addToast]
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

  const handleCommand = useCallback(
    async (command: string, args?: string) => {
      if (command === "new") {
        if (!appState) return;
        try {
          await newSession(appState.active_project);
          refreshState();
          setSelectedModel(null);
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

      if (!activeSessionId) return;
      try {
        await executeCommand(activeSessionId, command, args);
        refreshState();
      } catch {
        addToast(`Command /${command} failed`, "error");
      }
    },
    [activeSessionId, appState, refreshState, addToast]
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
      } catch {
        addToast("Failed to switch session", "error");
      }
    },
    [appState, refreshState, addToast]
  );

  const handleNewSession = useCallback(async () => {
    if (!appState) return;
    try {
      await newSession(appState.active_project);
      refreshState();
      setSelectedModel(null);
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

  // ── Keyboard shortcuts ────────────────────────────────

  useKeyboard([
    {
      key: "p",
      meta: true,
      shift: true,
      handler: () => setCommandPaletteOpen(true),
      description: "Command Palette",
    },
    {
      key: "'",
      meta: true,
      handler: () => setModelPickerOpen(true),
      description: "Model Picker",
    },
    {
      key: "b",
      meta: true,
      handler: () => setSidebarOpen((v) => !v),
      description: "Toggle Sidebar",
    },
    {
      key: "`",
      meta: true,
      handler: () => setTerminalOpen((v) => !v),
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
      handler: () => setNeovimOpen((v) => !v),
      description: "Toggle Neovim",
    },
    {
      key: "g",
      meta: true,
      shift: true,
      handler: () => setGitOpen((v) => !v),
      description: "Toggle Git",
    },
    {
      key: "Escape",
      handler: () => {
        if (commandPaletteOpen) setCommandPaletteOpen(false);
        else if (modelPickerOpen) setModelPickerOpen(false);
        else if (themeSelectorOpen) setThemeSelectorOpen(false);
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
          onClick={() => setMobileSidebarOpen(false)}
        />
      )}

      {/* Content area: sidebar + main + side panel */}
      <div className="chat-content">
        {/* Sidebar */}
        {sidebarOpen && (
          <>
            <div style={{ width: sidebarResize.size, flexShrink: 0 }}>
              <ChatSidebar
                projects={appState.projects}
                activeProject={appState.active_project}
                activeSessionId={activeSessionId}
                busySessions={busySessions}
                onSelectSession={handleSelectSession}
                onNewSession={handleNewSession}
                onSwitchProject={handleSwitchProject}
                isMobileOpen={mobileSidebarOpen}
                onClose={() => setMobileSidebarOpen(false)}
              />
            </div>
            {/* Sidebar resize handle */}
            <div {...sidebarResize.handleProps} />
          </>
        )}

        {/* Main chat area */}
        <div className="chat-main">
          {/* Mobile header */}
          <div className="chat-mobile-header">
            <button
              className="mobile-hamburger"
              onClick={() => setMobileSidebarOpen((v) => !v)}
            >
              {mobileSidebarOpen ? "\u2715" : "\u2630"}
            </button>
            <span className="mobile-project-name">
              {activeProject?.name || "opman"}
            </span>
            {sessionStatus === "busy" && <span className="dot busy" />}
          </div>

          {/* Message timeline */}
          <MessageTimeline
            messages={messages}
            sessionStatus={sessionStatus}
            activeSessionId={activeSessionId}
          />

          {/* Permission dock */}
          {permissions.length > 0 && (
            <PermissionDock
              permissions={permissions}
              onReply={handlePermissionReply}
            />
          )}

          {/* Question dock */}
          {questions.length > 0 && (
            <QuestionDock
              questions={questions}
              onReply={handleQuestionReply}
            />
          )}

          {/* Prompt input */}
          <PromptInput
            onSend={handleSend}
            onAbort={handleAbort}
            onCommand={handleCommand}
            onOpenModelPicker={() => setModelPickerOpen(true)}
            isBusy={sessionStatus === "busy"}
            isSending={sending}
            disabled={!activeSessionId}
            sessionId={activeSessionId}
            currentModel={currentModel}
          />

          {/* Terminal panel (collapsible, at bottom) */}
          {terminalOpen && (
            <>
              <div {...terminalResize.handleProps} />
              <div style={{ height: terminalResize.size, flexShrink: 0 }}>
                <TerminalPanel
                  sessionId={activeSessionId}
                  onClose={() => setTerminalOpen(false)}
                />
              </div>
            </>
          )}
        </div>

        {/* Side panel: Neovim or Git (right of main) */}
        {hasSidePanel && (
          <>
            {/* Side panel resize handle */}
            <div {...sidePanelResize.handleProps} />
            <div
              className="side-panel"
              style={{ width: sidePanelResize.size, flexShrink: 0 }}
            >
              {neovimOpen && (
                <div className="side-panel-section">
                  <div className="side-panel-header">
                    <FileCode size={14} />
                    <span>Neovim</span>
                    <button
                      className="side-panel-close"
                      onClick={() => setNeovimOpen(false)}
                    >
                      <X size={14} />
                    </button>
                  </div>
                  <div className="side-panel-body">
                    <XtermPanel
                      kind="neovim"
                      sessionId={activeSessionId || undefined}
                      focused={neovimOpen && !gitOpen}
                    />
                  </div>
                </div>
              )}
              {gitOpen && (
                <div className="side-panel-section">
                  <div className="side-panel-header">
                    <GitBranch size={14} />
                    <span>Git</span>
                    <button
                      className="side-panel-close"
                      onClick={() => setGitOpen(false)}
                    >
                      <X size={14} />
                    </button>
                  </div>
                  <div className="side-panel-body">
                    <XtermPanel
                      kind="git"
                      focused={gitOpen}
                    />
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
          onClose={() => setCommandPaletteOpen(false)}
          onCommand={handleCommand}
          onNewSession={handleNewSession}
          onToggleSidebar={() => setSidebarOpen((v) => !v)}
          onToggleTerminal={() => setTerminalOpen((v) => !v)}
          onOpenModelPicker={() => {
            setCommandPaletteOpen(false);
            setModelPickerOpen(true);
          }}
          sessionId={activeSessionId}
        />
      )}

      {/* Model picker modal */}
      {modelPickerOpen && (
        <ModelPickerModal
          onClose={() => setModelPickerOpen(false)}
          sessionId={activeSessionId}
          onModelSelected={handleModelSelected}
        />
      )}

      {/* Theme selector modal */}
      {themeSelectorOpen && (
        <ThemeSelectorModal
          onClose={() => setThemeSelectorOpen(false)}
          onThemeApplied={(colors) => {
            applyTheme(colors);
            addToast("Theme applied", "success");
          }}
        />
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
        onToggleSidebar={() => setSidebarOpen((v) => !v)}
        onToggleTerminal={() => setTerminalOpen((v) => !v)}
        onToggleNeovim={() => setNeovimOpen((v) => !v)}
        onToggleGit={() => setGitOpen((v) => !v)}
        onOpenCommandPalette={() => setCommandPaletteOpen(true)}
      />

      {/* Toast notifications */}
      <ToastContainer toasts={toasts} onDismiss={removeToast} />

      {/* Mobile bottom navbar (visible <768px) */}
      <div className="mobile-bottom-nav">
        <div className="mobile-bottom-nav-inner">
          <button
            className={`mobile-nav-btn ${mobileSidebarOpen ? "active" : ""}`}
            onClick={() => setMobileSidebarOpen((v) => !v)}
          >
            <PanelLeft size={18} />
            <span className="mobile-nav-btn-label">Sidebar</span>
          </button>
          <button
            className={`mobile-nav-btn ${terminalOpen ? "active" : ""}`}
            onClick={() => setTerminalOpen((v) => !v)}
          >
            <Terminal size={18} />
            <span className="mobile-nav-btn-label">Terminal</span>
          </button>
          <button
            className={`mobile-nav-btn ${neovimOpen ? "active" : ""}`}
            onClick={() => setNeovimOpen((v) => !v)}
          >
            <FileCode size={18} />
            <span className="mobile-nav-btn-label">Neovim</span>
          </button>
          <button
            className={`mobile-nav-btn ${gitOpen ? "active" : ""}`}
            onClick={() => setGitOpen((v) => !v)}
          >
            <GitBranch size={18} />
            <span className="mobile-nav-btn-label">Git</span>
          </button>
          <button
            className="mobile-nav-btn"
            onClick={() => setCommandPaletteOpen(true)}
          >
            <Command size={18} />
            <span className="mobile-nav-btn-label">Commands</span>
          </button>
        </div>
      </div>
    </div>
  );
}
