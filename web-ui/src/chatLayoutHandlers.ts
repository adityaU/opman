import {
  sendMessage, abortSession, executeCommand,
  replyPermission, replyQuestion, rejectQuestion, newSession, switchProject,
  fetchAppState,
} from "./api";
import type { ImageAttachment, PersonalMemoryItem } from "./api";
import type { Message } from "./types";

/* ── Deps interface ─────────────────────────────────────── */

export interface HandlerDeps {
  activeSessionId: string | null;
  /** URL-derived active project index — sole source of truth. */
  activeProjectIndex: number;
  appState: any;
  selectedModel: any;
  selectedAgent: string;
  sending: boolean;
  activeMemoryItems: PersonalMemoryItem[];
  setSending: (v: boolean, sessionId?: string) => void;
  setSelectedModel: (m: any) => void;
  setSelectedAgent: (a: string) => void;
  setMobileInputHidden: (v: boolean) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  addOptimisticMessage: (text: string, images?: ImageAttachment[]) => void;
  clearOptimistic: () => void;
  refreshState: () => void;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  setMobileSidebarOpen: (v: boolean) => void;
  /** Close mobile sidebar without history.back() — prevents undoing the session URL pushState. */
  closeMobileSidebarSilent: () => void;
  /** Navigate to a session via URL (single source of truth). */
  setUrlSession: (sessionId: string, projectIdx: number) => void;
  openModal: (name: string) => void;
  /** Signal that the next SSE session change is expected (blocks rogue session switches). */
  expectSessionSwitch: () => void;
  /** Open memory modal showing all memories (for /memory command). */
  openMemoryAll: () => void;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  toggleDebug: () => void;
  toggleSplitView: () => void;
  /** Read current messages without including them in memo deps. */
  getMessages: () => Message[];
}

/* ── Pure helper ────────────────────────────────────────── */

export function injectMemoryGuidance(text: string, memoryItems: PersonalMemoryItem[]): string {
  if (memoryItems.length === 0) return text;
  const guidance = memoryItems
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
  health: "processHealth", "process-health": "processHealth",
  "auto-open": "autoOpen", autoopen: "autoOpen",
};

const TOGGLE_COMMANDS = new Set(["terminal", "neovim", "nvim", "git", "split-view", "debug"]);

export const LOCAL_COMMANDS = new Set([
  "new", "cancel", "copy", ...Object.keys(MODAL_COMMANDS), ...TOGGLE_COMMANDS,
]);

/* ── Factory functions ──────────────────────────────────── */

export function createHandleSend(deps: HandlerDeps) {
  return async (text: string, images?: ImageAttachment[], fileContext?: string): Promise<boolean> => {
    const sid = deps.activeSessionId;
    if (!sid) return false;
    if (typeof window !== "undefined" && window.innerWidth < 768) {
      deps.setMobileInputHidden(true);
    }
    // Prepend file context (from @file mentions) before memory guidance
    const fullText = fileContext ? fileContext + text : text;
    const enrichedText = injectMemoryGuidance(fullText, deps.activeMemoryItems);
    // Show optimistic message with the full enriched text so memory pill renders immediately
    deps.addOptimisticMessage(enrichedText, images);
    try {
      await sendMessage(
        sid,
        enrichedText,
        deps.selectedModel ?? undefined, images,
        deps.selectedAgent || undefined,
      );
      return true;
    } catch {
      deps.clearOptimistic();
      deps.addToast("Failed to send message", "error");
      return false;
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
      const projectIdx = deps.activeProjectIndex;
        const resp = await newSession(projectIdx);
        // URL is the single source of truth — triggers beginSessionSwitch + API calls
        deps.setUrlSession(resp.session_id, projectIdx);
        deps.setSelectedModel(null);
        deps.setSelectedAgent("");
        deps.addToast("New session created", "success");
      } catch {
        deps.addToast("Failed to create session", "error");
      }
      return;
    }

    // /copy — copy session transcript to clipboard
    if (command === "copy") {
      const msgs = deps.getMessages();
      if (msgs.length === 0) { deps.addToast("Nothing to copy", "warning"); return; }
      const lines: string[] = [];
      for (const msg of msgs) {
        const role = msg.info.role === "user" ? "User" : "Assistant";
        for (const part of msg.parts) {
          if (part.type === "text" && part.text) lines.push(`## ${role}\n\n${part.text}`);
        }
      }
      if (lines.length === 0) { deps.addToast("No text content to copy", "warning"); return; }
      try {
        await navigator.clipboard.writeText(lines.join("\n\n---\n\n"));
        deps.addToast("Session transcript copied to clipboard", "success");
      } catch {
        deps.addToast("Clipboard access denied", "error");
      }
      return;
    }

    // Toggle panels
    if (command === "terminal") { deps.toggleTerminal(); return; }
    if (command === "neovim" || command === "nvim") { deps.toggleNeovim(); return; }
    if (command === "git") { deps.toggleGit(); return; }
    if (command === "debug") { deps.toggleDebug(); return; }
    if (command === "split-view") { deps.toggleSplitView(); return; }

    // Modal commands (models, theme, sessions, settings, etc.)
    // /memory from command palette shows ALL memories (not scoped)
    if (command === "memory") { deps.openMemoryAll(); return; }
    const modalName = MODAL_COMMANDS[command];
    if (modalName) { deps.openModal(modalName); return; }

    // Fallback: server-side command
    if (!deps.activeSessionId) return;
    try {
      await executeCommand(deps.activeSessionId, command, args);
      deps.refreshState();
    } catch (err: unknown) {
      const detail = err instanceof Error ? err.message : "";
      deps.addToast(detail || `Command /${command} failed`, "error");
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
    // Optimistic: remove from UI immediately
    deps.clearQuestion(requestId);
    try {
      await replyQuestion(requestId, answers);
    } catch {
      deps.addToast("Failed to send answer", "error");
    }
  };
}

export function createHandleQuestionDismiss(deps: HandlerDeps) {
  return async (requestId: string) => {
    // Optimistic: remove from UI immediately
    deps.clearQuestion(requestId);
    try {
      await rejectQuestion(requestId);
    } catch {
      deps.addToast("Failed to dismiss question", "error");
    }
  };
}

export function createHandleSelectSession(deps: HandlerDeps) {
  return (sessionId: string, projectIdx: number) => {
    if (!deps.appState) return;
    // Close mobile sidebar without history.back() so the subsequent
    // pushState for the new session URL isn't undone by the async back().
    deps.closeMobileSidebarSilent();
    // URL is the single source of truth — this triggers beginSessionSwitch + API calls
    deps.setUrlSession(sessionId, projectIdx);
    deps.setSelectedModel(null);
    deps.setSelectedAgent("");
  };
}

export function createHandleNewSession(deps: HandlerDeps) {
  return async () => {
    if (!deps.appState) return;
    try {
      const projectIdx = deps.activeProjectIndex;
      const resp = await newSession(projectIdx);
      // URL is the single source of truth — triggers beginSessionSwitch + API calls
      deps.setUrlSession(resp.session_id, projectIdx);
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
      await switchProject(index);
      // Fetch fresh state to discover the new project's active session
      const freshState = await fetchAppState();
      const proj = freshState.projects[index];
      const newSid = proj?.active_session ?? null;
      if (newSid) {
        // URL is the single source of truth — triggers beginSessionSwitch + API calls
        deps.setUrlSession(newSid, index);
      }
      deps.setSelectedModel(null);
      deps.setSelectedAgent("");
    } catch {
      deps.addToast("Failed to switch project", "error");
    }
  };
}

export function createHandleModelSelected(deps: HandlerDeps) {
  return (modelId: string, providerId: string) => {
    deps.expectSessionSwitch();
    deps.setSelectedModel({ providerID: providerId, modelID: modelId });
    deps.addToast(`Model switched to ${modelId}`, "success");
  };
}
