import {
  sendMessage, abortSession, executeCommand,
  replyPermission, replyQuestion, rejectQuestion, newSession, switchProject,
  fetchAppState,
} from "./api";
import type { ImageAttachment, PersonalMemoryItem } from "./api";
import type { Message } from "./types";
import { isMobileViewport } from "./hooks/useIsMobile";

/* ── Deps interface ─────────────────────────────────────── */

export interface HandlerDeps {
  activeSessionId: string | null;
  /** URL-derived active project index — sole source of truth. */
  activeProjectIndex: number;
  appState: any;
  selectedModel: any;
  selectedAgent: string;
  /** Runner a session created right now should be created with. */
  runnerForNewSession: string;
  /**
   * Runner the user explicitly picked for the active session, when it differs
   * from the one that session already uses. Non-null means "hand this session
   * off" — it is the only value that may travel with a send on an existing
   * session.
   */
  runnerSwitch: string | null;
  selectedEffort: string | null;
  selectedPermission: string;
  sending: boolean;
  activeMemoryItems: PersonalMemoryItem[];
  setSending: (v: boolean, sessionId?: string) => void;
  setSelectedModel: (m: any) => void;
  setSelectedAgent: (a: string) => void;
  /** Forget the user's runner pick (on session switch). */
  clearRunnerChoice: () => void;
  /**
   * Re-anchor the runner pick to a session that now owns it, so the composer
   * keeps showing that runner without re-requesting a switch on the next send.
   */
  bindRunnerChoice: (sessionId: string, runner: string) => void;
  setMobileInputHidden: (v: boolean) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  addOptimisticMessage: (text: string, images?: ImageAttachment[], sessionId?: string | null) => string | null;
  clearOptimistic: (sessionId?: string | null, id?: string) => void;
  refreshState: () => void;
  /** Refresh the active transcript after runner adapters complete synchronously. */
  refreshMessages: (sessionId?: string | null, options?: { adoptView?: boolean }) => Promise<void>;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  setMobileSidebarOpen: (v: boolean) => void;
  /** Close mobile sidebar without history.back() — prevents undoing the session URL pushState. */
  closeMobileSidebarSilent: () => void;
  /** Navigate to a session via URL (single source of truth). */
  setUrlSession: (sessionId: string | null, projectIdx: number) => void;
  openModal: (name: string) => void;
  /** Signal that the next SSE session change is expected (for real session switches). */
  expectSessionSwitch: () => void;
  /** Temporarily block background SSE-driven session adoption for non-switch actions. */
  blockSessionAdoption: (ms?: number) => void;
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

/*
 * Session instructions used to be prepended here, to every outgoing message.
 * The server owns that now: it delivers them on a session's opening turn only
 * (and on the handoff message that opens a taken-over session), so they are not
 * re-sent and re-billed on every turn, and every client — queue flushes, the
 * kanban launcher, another browser tab — gets the same treatment.
 */

/* ── Command → modal mapping ────────────────────────────── */

const MODAL_COMMANDS: Record<string, string> = {
  models: "modelPicker", model: "modelPicker", agent: "agentPicker",
  theme: "themeSelector", keys: "cheatsheet", keybindings: "cheatsheet",
  todos: "todoPanel", sessions: "sessionSelector", context: "contextInput",
  settings: "settings", watcher: "watcher",
  "context-window": "contextWindow", "diff-review": "diffReview",
  search: "searchBar", "cross-search": "crossSearch",
  "notification-prefs": "notificationPrefs",
  memory: "memory", autonomy: "autonomy", routines: "routines",
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
    let sid = deps.activeSessionId;
    // Whether this send is what brings the session into existence. Only then may
    // the follow-up refresh take over the view — see the refresh call below.
    const createdSession = !sid;
    // A session that exists already belongs to a runner. Re-stating a runner on
    // every send is what used to fork the conversation: any drift in the value
    // the UI inferred read as a switch request, and the backend answered with a
    // handoff session. Name a runner only when creating the session, or when the
    // user deliberately switched.
    const runnerForNewSession = deps.runnerForNewSession;
    const runnerForSend = sid ? deps.runnerSwitch : runnerForNewSession;
    if (!sid) {
      // Creating the session is a round-trip of its own. Flag the send before
      // it starts so the composer and transcript can report progress instead of
      // sitting on an empty screen until the session exists.
      deps.setSending(true);
      try {
        const created = await newSession(deps.activeProjectIndex, runnerForNewSession);
        sid = created.session_id;
        deps.bindRunnerChoice(sid, runnerForNewSession);
        deps.setUrlSession(sid, deps.activeProjectIndex);
      } catch {
        deps.setSending(false);
        deps.addToast("Failed to create session", "error");
        return false;
      }
    }
    if (typeof window !== "undefined" && isMobileViewport()) {
      deps.setMobileInputHidden(true);
    }
    // Reserve this session before starting the request so rapid submits cannot
    // race and start competing turns on the same conversation.
    deps.setSending(true, sid);
    // File context (from @file mentions) still belongs to the message itself.
    const fullText = fileContext ? fileContext + text : text;
    const placeholderId = deps.addOptimisticMessage(fullText, images, sid);
    try {
      const result = await sendMessage(
        sid,
        fullText,
        deps.selectedModel ?? undefined, images,
        deps.selectedAgent || undefined,
        runnerForSend || undefined,
        deps.selectedEffort || undefined,
        deps.selectedPermission || undefined,
      );
      const handoff = result as { session_id?: string; switched?: boolean; runner?: string } | undefined;
      // HTTP runners normally update the transcript through SSE. The Codex
      // adapter completes synchronously, so refresh here also covers that
      // runner and makes a handoff immediately visible.
      // A send does not entitle its session to the screen. If the user opened
      // another conversation while this request was in flight, refresh that
      // session's transcript in the background instead of pulling them back.
      await deps.refreshMessages(sid, { adoptView: createdSession });
      if (handoff?.switched && handoff.session_id) {
        deps.setUrlSession(handoff.session_id, deps.activeProjectIndex);
        // Move the pick onto the session that now serves it. Leaving it armed on
        // the old session would re-request the switch and fork again.
        deps.bindRunnerChoice(handoff.session_id, handoff.runner || deps.runnerForNewSession);
        deps.addToast(`Session handed off to ${handoff.runner || "new runner"}`, "success");
      }
      return true;
    } catch {
      // Retire only this send's placeholder: a blanket purge would also drop a
      // queued prompt on another session that never failed.
      if (placeholderId) deps.clearOptimistic(sid, placeholderId);
      deps.addToast("Failed to send message", "error");
      return false;
    } finally {
      deps.setSending(false, sid);
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
        deps.setUrlSession(null, projectIdx);
        // URL is the single source of truth — triggers beginSessionSwitch + API calls
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
    deps.clearRunnerChoice();
  };
}

export function createHandleNewSession(deps: HandlerDeps) {
  return async () => {
    if (!deps.appState) return;
    try {
      const projectIdx = deps.activeProjectIndex;
      deps.setUrlSession(null, projectIdx);
      deps.setSelectedModel(null);
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
    deps.blockSessionAdoption();
    deps.setSelectedModel({ providerID: providerId, modelID: modelId });
    deps.addToast(`Model switched to ${modelId}`, "success");
  };
}

/**
 * Runner a brand-new session should be created with.
 *
 * `appState.backend` names the CLI opman wraps, not a runner — both claude
 * engines report "claude-code" — so it can never identify one. `default_runner`
 * is the server's own answer; trust it and nothing else.
 */
export function defaultRunner(appState: any): string {
  return appState?.default_runner || "opencode";
}
