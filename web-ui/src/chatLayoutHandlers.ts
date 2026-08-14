import {
  sendMessage, abortSession, executeCommand,
  replyPermission, replyQuestion, rejectQuestion, newSession,
} from "./api";
import type { ImageAttachment } from "./api";
import type { Message } from "./types";
import { isMobileViewport } from "./hooks/useIsMobile";
import { findSlashCommand } from "./keybindings/commands";

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
  /** Close mobile sidebar without history.back() — prevents undoing the session URL pushState. */
  closeMobileSidebarSilent: () => void;
  /** Navigate to a session via URL (single source of truth). */
  setUrlSession: (sessionId: string | null, projectIdx: number) => void;
  /** Temporarily block background SSE-driven session adoption for non-switch actions. */
  blockSessionAdoption: (ms?: number) => void;
  /**
   * Run a registered command by id, reporting whether a mounted surface handled it.
   *
   * This is how a `/name` reaches its implementation: the slash resolves to a command id
   * and the keymap's own registry runs it, so the composer and the chord that share a
   * command share one implementation rather than two that can drift.
   */
  runCommandId: (id: string) => boolean;
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
        // The composer's configuration is part of creating the session, not of the send
        // that follows it: a session created bare reports its engine's own defaults, and
        // that row read back would overwrite the permission mode on screen.
        const created = await newSession(deps.activeProjectIndex, runnerForNewSession, {
          ...(deps.selectedModel ? { model: deps.selectedModel.modelID } : {}),
          ...(deps.selectedAgent ? { agent: deps.selectedAgent } : {}),
          ...(deps.selectedEffort ? { effort: deps.selectedEffort } : {}),
          ...(deps.selectedPermission ? { permissionMode: deps.selectedPermission } : {}),
        });
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

/**
 * Run a `/name` typed in the composer.
 *
 * One decision, made from the command registry: a name opman claims runs the command
 * registered under that id, and every other name is the runner's. There is deliberately no
 * list of agent commands to consult — an unknown name is forwarded, because the agent knows
 * its own vocabulary and opman does not.
 */
export function createHandleCommand(deps: HandlerDeps) {
  return async (command: string, args?: string) => {
    const local = findSlashCommand(command);
    if (local) {
      // A false result means the id has no mounted handler: the command is opman's, so
      // forwarding it to the agent would only send it a word it never claimed. Say so
      // instead of doing nothing visible.
      if (!deps.runCommandId(local.id)) {
        deps.addToast(`${local.title} is not available here`, "warning");
      }
      return;
    }

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

/** `/copy` — the transcript as markdown, on the clipboard. */
export function createHandleCopyTranscript(deps: HandlerDeps) {
  return async () => {
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
