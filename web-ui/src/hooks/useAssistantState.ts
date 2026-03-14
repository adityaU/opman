import { useState, useEffect, useRef, useCallback } from "react";
import type {
  PersonalMemoryItem, AutonomyMode, Mission,
  RoutineDefinition, RoutineRunRecord, DelegatedWorkItem, WorkspaceSnapshot,
} from "../api";
import {
  fetchPersonalMemory, fetchAutonomySettings, fetchRoutines,
  fetchMissions, fetchDelegatedWork, fetchWorkspaces,
  computeRecommendations, computeResumeBriefing,
  fetchActiveMemory,
} from "../api";
import type { AssistantRecommendation, ResumeBriefing } from "../api/intelligence";
import type { PermissionRequest, QuestionRequest } from "../types";
import { toPermissionInputs, toQuestionInputs, toSignalInputs } from "./intelligenceAdapters";

/** Signal stored client-side (from SSE events / watcher triggers / notifications). */
export interface AssistantSignal {
  id: string;
  kind: "session_complete" | "watcher_trigger" | "permission_request" | "question" | "file_edit";
  title: string;
  body: string;
  createdAt: number;
  sessionId?: string | null;
}

export interface UseAssistantStateOptions {
  appState: any; activeSessionId: string | null;
  activeProject: number; sessionStatus: string;
  permissions: PermissionRequest[]; questions: QuestionRequest[];
  liveActivityEvents: any[]; watcherStatus: any;
  memoryOpen: boolean; autonomyOpen: boolean;
  routinesOpen: boolean; missionsOpen: boolean;
  delegationOpen: boolean; workspaceManagerOpen: boolean;
  assistantCenterOpen: boolean;
}

export interface UseAssistantStateCallbacks {
  onOpenAssistantCenter: () => void;
}

export function useAssistantState(
  opts: UseAssistantStateOptions,
  cbs: UseAssistantStateCallbacks,
) {
  const {
    appState, activeSessionId, activeProject,
    permissions, questions, liveActivityEvents,
    memoryOpen, autonomyOpen, routinesOpen, missionsOpen,
    delegationOpen, workspaceManagerOpen, assistantCenterOpen,
  } = opts;

  // ── State ──
  const [assistantSignals, setAssistantSignals] = useState<AssistantSignal[]>([]);
  const [personalMemory, setPersonalMemory] = useState<PersonalMemoryItem[]>([]);
  const [autonomyMode, setAutonomyMode] = useState<AutonomyMode>("observe");
  const [missionCache, setMissionCache] = useState<Mission[]>([]);
  const [routineCache, setRoutineCache] = useState<RoutineDefinition[]>([]);
  const [routineRunCache, setRoutineRunCache] = useState<RoutineRunRecord[]>([]);
  const [delegatedWorkCache, setDelegatedWorkCache] = useState<DelegatedWorkItem[]>([]);
  const [workspaceCache, setWorkspaceCache] = useState<WorkspaceSnapshot[]>([]);
  const [resumeBriefing, setResumeBriefing] = useState<ResumeBriefing | null>(null);
  const [latestDailySummary, setLatestDailySummary] = useState<string | null>(null);
  const [activeWorkspaceName, setActiveWorkspaceName] = useState<string | null>(null);
  const [activeMemoryItems, setActiveMemoryItems] = useState<PersonalMemoryItem[]>([]);
  const [assistantPulse, setAssistantPulse] = useState<AssistantRecommendation | null>(null);

  const lastVisibleAtRef = useRef(Date.now());

  // ── Shared routine refresh helper ──
  const refreshRoutines = useCallback(() => {
    fetchRoutines()
      .then((resp) => {
        setRoutineCache(resp.routines ?? []);
        setRoutineRunCache(resp.runs ?? []);
      })
      .catch(() => {});
  }, []);

  // ── Backend-driven active memory ──
  useEffect(() => {
    fetchActiveMemory(appState?.active_project, activeSessionId)
      .then((resp) => setActiveMemoryItems(Array.isArray(resp?.memory) ? (resp.memory as PersonalMemoryItem[]).filter(Boolean) : []))
      .catch(() => {});
  }, [personalMemory, appState, activeSessionId]);

  // ── Backend-driven recommendations (pulse) ──
  useEffect(() => {
    computeRecommendations({
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
    })
      .then((resp) => setAssistantPulse(resp?.recommendations?.[0] ?? null))
      .catch(() => {});
  }, [autonomyMode, missionCache, delegatedWorkCache, activeMemoryItems, routineCache, permissions, questions, workspaceCache]);

  // ── Resume briefing on visibility change ──
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.hidden) {
        lastVisibleAtRef.current = Date.now();
        return;
      }
      const awayMs = Date.now() - lastVisibleAtRef.current;
      if (awayMs < 5 * 60 * 1000) return;

      computeResumeBriefing({
        active_session_id: activeSessionId,
        permissions: toPermissionInputs(permissions),
        questions: toQuestionInputs(questions),
        signals: toSignalInputs(assistantSignals),
      })
        .then((briefing) => {
          if (briefing && autonomyMode !== "observe") {
            setResumeBriefing(briefing);
            cbs.onOpenAssistantCenter();
          }
        })
        .catch(() => {});
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => document.removeEventListener("visibilitychange", handleVisibilityChange);
  }, [activeSessionId, missionCache, permissions, questions, liveActivityEvents, assistantSignals, autonomyMode]);

  // ── Data-fetching effects ──
  useEffect(() => {
    fetchPersonalMemory()
      .then((resp) => setPersonalMemory((resp?.memory ?? []).filter(Boolean)))
      .catch(() => {});
  }, [memoryOpen]);

  useEffect(() => {
    fetchAutonomySettings()
      .then((settings) => setAutonomyMode(settings.mode ?? "observe"))
      .catch(() => {});
  }, [autonomyOpen]);

  useEffect(() => {
    refreshRoutines();
  }, [routinesOpen, assistantCenterOpen, refreshRoutines]);

  useEffect(() => {
    fetchMissions()
      .then((resp) => setMissionCache(resp.missions ?? []))
      .catch(() => {});
  }, [missionsOpen, routinesOpen, assistantCenterOpen]);

  // Live mission updates from SSE
  useEffect(() => {
    const handler = (e: Event) => {
      const mission = (e as CustomEvent).detail as Mission;
      if (!mission?.id) return;
      setMissionCache((prev) => {
        const idx = prev.findIndex((m) => m.id === mission.id);
        if (idx >= 0) {
          const next = [...prev];
          next[idx] = mission;
          return next;
        }
        return [mission, ...prev];
      });
    };
    window.addEventListener("opman:mission-updated", handler);
    return () => window.removeEventListener("opman:mission-updated", handler);
  }, []);

  // Live routine updates from SSE — refetch routine cache when backend signals a change
  useEffect(() => {
    const handler = () => { refreshRoutines(); };
    window.addEventListener("opman:routine-updated", handler);
    return () => window.removeEventListener("opman:routine-updated", handler);
  }, [refreshRoutines]);

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

  // ── NOTE: on_session_idle and daily_summary routines are now fully ──
  // ── backend-driven. The frontend only displays status; it never ──
  // ── initiates idle-triggered automations. This ensures reliable ──
  // ── execution even when the browser tab is backgrounded. ──

  // ── Derive latest daily summary from run cache ──
  useEffect(() => {
    const dailyRoutineIds = new Set(
      routineCache.filter((item) => item.trigger === "daily_summary").map((item) => item.id),
    );
    const latest = routineRunCache.find((run) => dailyRoutineIds.has(run.routine_id));
    if (latest?.summary) {
      setLatestDailySummary(latest.summary);
    }
  }, [routineCache, routineRunCache]);

  return {
    assistantSignals, setAssistantSignals,
    personalMemory, setPersonalMemory,
    autonomyMode, setAutonomyMode,
    missionCache, setMissionCache,
    routineCache, setRoutineCache,
    routineRunCache, setRoutineRunCache,
    delegatedWorkCache, setDelegatedWorkCache,
    workspaceCache, setWorkspaceCache,
    resumeBriefing, setResumeBriefing,
    latestDailySummary, setLatestDailySummary,
    activeWorkspaceName, setActiveWorkspaceName,
    activeMemoryItems,
    assistantPulse,
  };
}
