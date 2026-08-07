import { useEffect, useRef } from "react";
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
import type { SessionStatus } from "./sse/types";

export interface UseNotificationSignalsOptions {
  activeSessionId: string | null;
  sessionStatus: SessionStatus;
  autonomyMode: AutonomyMode;
  watcherStatus: any;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  crossSessionPermissions: PermissionRequest[];
  crossSessionQuestions: QuestionRequest[];
  fileEditCount: number;
}

/**
 * Handles presence registration and browser notifications for session events.
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
    const wasBusy = prevStatusRef.current.type !== "idle";
    const sameSession = prevSessionRef.current === activeSessionId;
    prevStatusRef.current = sessionStatus;
    prevSessionRef.current = activeSessionId;

    if (!prefs.enabled) return;

    // Only notify when the *same* session transitioned from busy→idle.
    if (sessionStatus.type === "idle" && wasBusy && sameSession && prefs.session_complete && autonomyMode !== "observe") {
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
