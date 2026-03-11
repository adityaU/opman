import { useEffect } from "react";
import type { AssistantSignal } from "./useAssistantState";
import {
  getClientId,
  loadNotificationPrefs,
  showNotification,
} from "../NotificationManager";
import type { NotifyEventKind } from "../NotificationManager";
import {
  registerPresence,
  deregisterPresence,
  updateAutonomySettings,
} from "../api";
import type { AutonomyMode } from "../api";

export interface UseNotificationSignalsOptions {
  activeSessionId: string | null;
  sessionStatus: string;
  autonomyMode: AutonomyMode;
  watcherStatus: any;
  setAssistantSignals: React.Dispatch<React.SetStateAction<AssistantSignal[]>>;
}

/**
 * Handles presence registration, browser notifications for session events,
 * and watcher-triggered assistant signals.
 */
export function useNotificationSignals(opts: UseNotificationSignalsOptions): void {
  const {
    activeSessionId,
    sessionStatus,
    autonomyMode,
    watcherStatus,
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

  // ── Browser notifications for session events ──
  useEffect(() => {
    const prefs = loadNotificationPrefs();
    if (!prefs.enabled) return;

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
        () => window.focus(),
      );
    }
  }, [sessionStatus, activeSessionId, autonomyMode]);

  // ── Watcher-triggered signals ──
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
}
