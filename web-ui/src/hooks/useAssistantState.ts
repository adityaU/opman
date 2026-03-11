import { useState, useEffect, useRef } from "react";
import type {
  PersonalMemoryItem, AutonomyMode, Mission,
  RoutineDefinition, RoutineRunRecord, DelegatedWorkItem, WorkspaceSnapshot,
} from "../api";
import {
  fetchPersonalMemory, fetchAutonomySettings, fetchRoutines,
  fetchMissions, fetchDelegatedWork, fetchWorkspaces, runRoutine,
  computeRecommendations, computeResumeBriefing, computeDailySummary,
  fetchActiveMemory,
} from "../api";
import type { AssistantRecommendation, ResumeBriefing, SignalInput } from "../api/intelligence";
import type { PermissionRequest, QuestionRequest } from "../types";
import { toPermissionInputs, toQuestionInputs, toSignalInputs } from "./intelligenceAdapters";

/** Signal stored client-side (from SSE events / watcher triggers). */
export interface AssistantSignal {
  id: string;
  kind: "session_complete" | "watcher_trigger";
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
  onOpenInbox: () => void; onOpenActivityFeed: () => void;
  onOpenMissions: () => void; onOpenAssistantCenter: () => void;
}

export function useAssistantState(
  opts: UseAssistantStateOptions,
  cbs: UseAssistantStateCallbacks,
) {
  const {
    appState, activeSessionId, activeProject, sessionStatus,
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
  const [executedAutoRoutineIds, setExecutedAutoRoutineIds] = useState<string[]>([]);
  const [resumeBriefing, setResumeBriefing] = useState<ResumeBriefing | null>(null);
  const [latestDailySummary, setLatestDailySummary] = useState<string | null>(null);
  const [activeWorkspaceName, setActiveWorkspaceName] = useState<string | null>(null);
  const [activeMemoryItems, setActiveMemoryItems] = useState<PersonalMemoryItem[]>([]);
  const [assistantPulse, setAssistantPulse] = useState<AssistantRecommendation | null>(null);

  const lastVisibleAtRef = useRef(Date.now());

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

  // ── Auto-routine execution (backend computes summary) ──
  useEffect(() => {
    if (autonomyMode !== "continue" && autonomyMode !== "autonomous") return;
    if (sessionStatus !== "idle" || !activeSessionId) return;

    const routine = routineCache.find(
      (item) => item.trigger === "on_session_idle" && item.session_id === activeSessionId,
    );
    if (!routine) return;
    if (executedAutoRoutineIds.includes(`${routine.id}:${activeSessionId}`)) return;

    // Ask backend to compute the routine summary
    computeDailySummary({
      routine_id: routine.id,
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
      signals: toSignalInputs(assistantSignals),
    })
      .then((resp) => runRoutine(routine.id, { summary: resp.summary }))
      .catch(() => runRoutine(routine.id, {}));

    if (routine.action === "open_inbox") {
      cbs.onOpenInbox();
    } else if (routine.action === "open_activity_feed") {
      cbs.onOpenActivityFeed();
    } else if (routine.action === "review_mission" && routine.mission_id) {
      cbs.onOpenMissions();
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

  // ── Reset executed auto-routine IDs when session becomes busy ──
  useEffect(() => {
    if (sessionStatus === "busy") {
      setExecutedAutoRoutineIds([]);
    }
  }, [sessionStatus]);

  // ── Daily summary routine (backend-computed) ──
  useEffect(() => {
    if (autonomyMode !== "continue" && autonomyMode !== "autonomous") return;

    const routine = routineCache.find((item) => item.trigger === "daily_summary");
    if (!routine) return;

    const todayKey = new Date().toISOString().slice(0, 10);
    const ranToday = routineRunCache.some(
      (run) => run.routine_id === routine.id && run.created_at.slice(0, 10) === todayKey,
    );
    if (ranToday) return;

    computeDailySummary({
      routine_id: routine.id,
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
      signals: toSignalInputs(assistantSignals),
    })
      .then((resp) => {
        setLatestDailySummary(resp.summary);
        return runRoutine(routine.id, { summary: resp.summary });
      })
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
          body: "Computing daily summary...",
          createdAt: Date.now(),
          sessionId: activeSessionId,
        },
        ...prev,
      ].slice(0, 25);
    });
  }, [autonomyMode, routineCache, routineRunCache, activeSessionId, missionCache, permissions, questions, assistantSignals]);

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
    executedAutoRoutineIds, setExecutedAutoRoutineIds,
    resumeBriefing, setResumeBriefing,
    latestDailySummary, setLatestDailySummary,
    activeWorkspaceName, setActiveWorkspaceName,
    activeMemoryItems,
    assistantPulse,
  };
}
