import { useEffect, useRef } from "react";
import type { AssistantSignal } from "./useAssistantState";
import {
  getClientId,
  loadNotificationPrefs,
  showNotification,
} from "../NotificationManager";
import {
  registerPresence,
  deregisterPresence,
} from "../api";
import type { AutonomyMode } from "../api";
import type { PermissionRequest, QuestionRequest } from "../types";

export interface UseNotificationSignalsOptions {
  activeSessionId: string | null;
  sessionStatus: string;
  autonomyMode: AutonomyMode;
  watcherStatus: any;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  crossSessionPermissions: PermissionRequest[];
  crossSessionQuestions: QuestionRequest[];
  fileEditCount: number;
  setAssistantSignals: React.Dispatch<React.SetStateAction<AssistantSignal[]>>;
}

/**
 * Handles presence registration, browser notifications for session events,
 * and watcher-triggered assistant signals.
 *
 * Fires service-worker-backed notifications (with plain Notification API fallback)
 * for: session_complete, permission_request, question, watcher_trigger, file_edit.
 */
export function useNotificationSignals(opts: UseNotificationSignalsOptions): void {
  const {
    activeSessionId,
    sessionStatus,
    autonomyMode,
    watcherStatus,
    permissions,
    questions,
    crossSessionPermissions,
    crossSessionQuestions,
    fileEditCount,
    setAssistantSignals,
  } = opts;

  // ── Presence registration + heartbeat ──
  useEffect(() => {
    const clientId = getClientId();
    const interfaceType = "web";

    registerPresence(clientId, interfaceType, activeSessionId ?? undefined).catch(() => {});

    const interval = setInterval(() => {
      registerPresence(clientId, interfaceType, activeSessionId ?? undefined).catch(() => {});
    }, 30000);

    return () => {
      clearInterval(interval);
      deregisterPresence(clientId).catch(() => {});
    };
  }, [activeSessionId]);

  // ── Browser notifications for session completion ──
  // Track previous status so we only fire on a genuine busy→idle transition,
  // not when e.g. the user switches to an already-idle session.
  const prevStatusRef = useRef(sessionStatus);
  const prevSessionRef = useRef(activeSessionId);
  useEffect(() => {
    const prefs = loadNotificationPrefs();
    const wasBusy = prevStatusRef.current === "busy";
    const sameSession = prevSessionRef.current === activeSessionId;
    prevStatusRef.current = sessionStatus;
    prevSessionRef.current = activeSessionId;

    if (!prefs.enabled) return;

    // Only notify when the *same* session transitioned from busy→idle.
    if (sessionStatus === "idle" && wasBusy && sameSession && prefs.session_complete && autonomyMode !== "observe") {
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
        "session_complete",
        "Session Complete",
        "AI session has finished processing",
        prefs,
        () => window.focus(),
        activeSessionId,
      );
    }
  }, [sessionStatus, activeSessionId, autonomyMode]);

  // ── Watcher-triggered signals + notifications ──
  useEffect(() => {
    if (!watcherStatus || watcherStatus.action !== "triggered" || autonomyMode === "observe") return;
    const prefs = loadNotificationPrefs();
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
    showNotification(
      "watcher_trigger",
      "Watcher Triggered",
      "A watched session auto-continued and may need review.",
      prefs,
      () => window.focus(),
      watcherStatus.session_id,
    );
  }, [watcherStatus, autonomyMode]);

  // ── Permission request notifications ──
  // Track already-notified IDs so we don't re-fire on re-render.
  const notifiedPermIdsRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    const prefs = loadNotificationPrefs();
    const allPerms = [...permissions, ...crossSessionPermissions];
    for (const perm of allPerms) {
      if (notifiedPermIdsRef.current.has(perm.id)) continue;
      notifiedPermIdsRef.current.add(perm.id);

      const label = perm.toolName || "Permission";
      setAssistantSignals((prev) => {
        const next: AssistantSignal = {
          id: `permission-request:${perm.id}:${Date.now()}`,
          kind: "permission_request",
          title: "Permission requested",
          body: `Tool "${label}" needs approval`,
          createdAt: Date.now(),
          sessionId: perm.sessionID,
        };
        return [next, ...prev].slice(0, 25);
      });
      showNotification(
        "permission_request",
        "Permission Requested",
        `Tool "${label}" needs approval`,
        prefs,
        () => window.focus(),
        perm.sessionID,
      );
    }
  }, [permissions, crossSessionPermissions]);

  // ── Question notifications ──
  const notifiedQuestionIdsRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    const prefs = loadNotificationPrefs();
    const allQs = [...questions, ...crossSessionQuestions];
    for (const q of allQs) {
      if (notifiedQuestionIdsRef.current.has(q.id)) continue;
      notifiedQuestionIdsRef.current.add(q.id);

      const label = q.title || "Question";
      setAssistantSignals((prev) => {
        const next: AssistantSignal = {
          id: `question:${q.id}:${Date.now()}`,
          kind: "question",
          title: "AI has a question",
          body: label,
          createdAt: Date.now(),
          sessionId: q.sessionID,
        };
        return [next, ...prev].slice(0, 25);
      });
      showNotification(
        "question",
        "AI Question",
        label,
        prefs,
        () => window.focus(),
        q.sessionID,
      );
    }
  }, [questions, crossSessionQuestions]);

  // ── File edit notifications ──
  const prevFileEditCountRef = useRef(fileEditCount);
  useEffect(() => {
    const prevCount = prevFileEditCountRef.current;
    prevFileEditCountRef.current = fileEditCount;

    // Only fire when there's an actual increment (not on initial mount or reset)
    if (fileEditCount <= prevCount || prevCount === 0 && fileEditCount === 0) return;

    const prefs = loadNotificationPrefs();
    showNotification(
      "file_edit",
      "File Edited",
      `${fileEditCount - prevCount} file(s) edited in the current session`,
      prefs,
      () => window.focus(),
      activeSessionId,
    );
    // No assistant signal for file edits — too noisy for the inbox.
  }, [fileEditCount, activeSessionId]);

  // ── Listen for NOTIFICATION_CLICK messages from the service worker ──
  useEffect(() => {
    if (!("serviceWorker" in navigator)) return;

    const handler = (event: MessageEvent) => {
      if (event.data?.type !== "NOTIFICATION_CLICK") return;
      // Focus the window (SW already called client.focus, but ensure)
      window.focus();
      // Future: could dispatch a custom event to navigate to the session
      // if (event.data.sessionId) { ... }
    };

    navigator.serviceWorker.addEventListener("message", handler);
    return () => navigator.serviceWorker.removeEventListener("message", handler);
  }, []);
}
