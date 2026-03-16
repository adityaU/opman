import {
  sendMessage, abortSession, executeCommand,
  replyPermission, replyQuestion, rejectQuestion, selectSession, newSession, switchProject,
} from "./api";
import type { ImageAttachment, PersonalMemoryItem } from "./api";

/* ── Deps interface ─────────────────────────────────────── */

export interface HandlerDeps {
  activeSessionId: string | null;
  appState: any;
  selectedModel: any;
  selectedAgent: string;
  sending: boolean;
  activeMemoryItems: PersonalMemoryItem[];
  setSending: (v: boolean) => void;
  setSelectedModel: (m: any) => void;
  setSelectedAgent: (a: string) => void;
  setMobileInputHidden: (v: boolean) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  addOptimisticMessage: (text: string) => void;
  refreshState: () => void;
  /** Signal that a user-initiated session switch is expected. */
  expectSessionSwitch: () => void;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  setMobileSidebarOpen: (v: boolean) => void;
  openModal: (name: string) => void;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  toggleSplitView: () => void;
}

/* ── Pure helper ────────────────────────────────────────── */

export function injectMemoryGuidance(text: string, memoryItems: PersonalMemoryItem[]): string {
  if (memoryItems.length === 0) return text;
  const guidance = memoryItems
    .slice(0, 5)
    .map((item) => `- ${item.label}: ${item.content}`)
    .join("\n");
  return `[Assistant memory in effect]\n${guidance}\n\n[User request]\n${text}`;
}

/* ── Command → modal mapping ────────────────────────────── */

const MODAL_COMMANDS: Record<string, string> = {
  models: "modelPicker", model: "modelPicker", agent: "agentPicker",
  theme: "themeSelector", keys: "cheatsheet", keybindings: "cheatsheet",
  todos: "todoPanel", sessions: "sessionSelector", context: "contextInput",
  settings: "settings", watcher: "watcher",
  "context-window": "contextWindow", "diff-review": "diffReview",
  search: "searchBar", "cross-search": "crossSearch",
  "session-graph": "sessionGraph", "session-dashboard": "sessionDashboard",
  "activity-feed": "activityFeed", "notification-prefs": "notificationPrefs",
  "assistant-center": "assistantCenter", inbox: "inbox",
  memory: "memory", autonomy: "autonomy", routines: "routines",
  delegation: "delegation", missions: "missions", workspaces: "workspaceManager",
  system: "systemMonitor", htop: "systemMonitor", monitor: "systemMonitor",
};

const TOGGLE_COMMANDS = new Set(["terminal", "neovim", "nvim", "git", "split-view"]);

export const LOCAL_COMMANDS = new Set([
  "new", "cancel", ...Object.keys(MODAL_COMMANDS), ...TOGGLE_COMMANDS,
]);

/* ── Factory functions ──────────────────────────────────── */

export function createHandleSend(deps: HandlerDeps) {
  return async (text: string, images?: ImageAttachment[]) => {
    if (!deps.activeSessionId || deps.sending) return;
    deps.setSending(true);
    deps.addOptimisticMessage(text);
    if (typeof window !== "undefined" && window.innerWidth < 768) {
      deps.setMobileInputHidden(true);
    }
    try {
      await sendMessage(
        deps.activeSessionId,
        injectMemoryGuidance(text, deps.activeMemoryItems),
        deps.selectedModel ?? undefined, images,
        deps.selectedAgent || undefined,
      );
    } catch {
      deps.addToast("Failed to send message", "error");
    } finally {
      deps.setSending(false);
    }
  };
}

export function createHandleAbort(deps: HandlerDeps) {
  return async () => {
    if (!deps.activeSessionId) return;
    try {
      await abortSession(deps.activeSessionId);
      deps.addToast("Session aborted", "info");
    } catch {
      deps.addToast("Failed to abort session", "error");
    }
  };
}

export function createHandleAgentChange(deps: HandlerDeps) {
  return async (agentId: string) => {
    deps.setSelectedAgent(agentId);
    if (deps.activeSessionId) {
      try { await executeCommand(deps.activeSessionId, "agent", agentId); } catch { /* best-effort */ }
    }
    deps.addToast(`Agent switched to ${agentId}`, "success");
  };
}

export function createHandleCommand(deps: HandlerDeps) {
  return async (command: string, args?: string) => {
    // /cancel — abort the running session (same as the Stop button)
    if (command === "cancel") {
      if (!deps.activeSessionId) return;
      try {
        await abortSession(deps.activeSessionId);
        deps.addToast("Session cancelled", "info");
      } catch {
        deps.addToast("Failed to cancel session", "error");
      }
      return;
    }

    // /new — create a new session
    if (command === "new") {
      if (!deps.appState) return;
      try {
        deps.expectSessionSwitch();
        await newSession(deps.appState.active_project);
        deps.refreshState();
        deps.setSelectedModel(null);
        deps.setSelectedAgent("");
        deps.addToast("New session created", "success");
      } catch {
        deps.addToast("Failed to create session", "error");
      }
      return;
    }

    // Toggle panels
    if (command === "terminal") { deps.toggleTerminal(); return; }
    if (command === "neovim" || command === "nvim") { deps.toggleNeovim(); return; }
    if (command === "git") { deps.toggleGit(); return; }
    if (command === "split-view") { deps.toggleSplitView(); return; }

    // Modal commands (models, theme, sessions, settings, etc.)
    const modalName = MODAL_COMMANDS[command];
    if (modalName) { deps.openModal(modalName); return; }

    // Fallback: server-side command
    if (!deps.activeSessionId) return;
    try {
      await executeCommand(deps.activeSessionId, command, args);
      deps.refreshState();
    } catch {
      deps.addToast(`Command /${command} failed`, "error");
    }
  };
}

export function createHandlePermissionReply(deps: HandlerDeps) {
  return async (requestId: string, reply: "once" | "always" | "reject") => {
    try {
      await replyPermission(requestId, reply);
      deps.clearPermission(requestId);
    } catch {
      deps.addToast("Failed to send permission reply", "error");
    }
  };
}

export function createHandleQuestionReply(deps: HandlerDeps) {
  return async (requestId: string, answers: string[][]) => {
    try {
      await replyQuestion(requestId, answers);
      deps.clearQuestion(requestId);
    } catch {
      deps.addToast("Failed to send answer", "error");
    }
  };
}

export function createHandleQuestionDismiss(deps: HandlerDeps) {
  return async (requestId: string) => {
    try {
      await rejectQuestion(requestId);
      deps.clearQuestion(requestId);
    } catch {
      deps.addToast("Failed to dismiss question", "error");
    }
  };
}

export function createHandleSelectSession(deps: HandlerDeps) {
  return async (sessionId: string, projectIdx: number) => {
    if (!deps.appState) return;
    try {
      deps.expectSessionSwitch();
      if (projectIdx !== deps.appState.active_project) {
        await switchProject(projectIdx);
      }
      await selectSession(projectIdx, sessionId);
      deps.refreshState();
      deps.setMobileSidebarOpen(false);
      deps.setSelectedModel(null);
      deps.setSelectedAgent("");
    } catch {
      deps.addToast("Failed to switch session", "error");
    }
  };
}

export function createHandleNewSession(deps: HandlerDeps) {
  return async () => {
    if (!deps.appState) return;
    try {
      deps.expectSessionSwitch();
      await newSession(deps.appState.active_project);
      deps.refreshState();
      deps.setSelectedModel(null);
      deps.setSelectedAgent("");
      deps.addToast("New session created", "success");
    } catch {
      deps.addToast("Failed to create session", "error");
    }
  };
}

export function createHandleSwitchProject(deps: HandlerDeps) {
  return async (index: number) => {
    try {
      deps.expectSessionSwitch();
      await switchProject(index);
      deps.refreshState();
      deps.setSelectedModel(null);
      deps.setSelectedAgent("");
    } catch {
      deps.addToast("Failed to switch project", "error");
    }
  };
}

export function createHandleModelSelected(deps: HandlerDeps) {
  return (modelId: string, providerId: string) => {
    deps.setSelectedModel({ providerID: providerId, modelID: modelId });
    deps.addToast(`Model switched to ${modelId}`, "success");
  };
}
