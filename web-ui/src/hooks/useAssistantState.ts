import { useState, useEffect, useRef, useCallback } from "react";
import type {
  PersonalMemoryItem, AutonomyMode,
  RoutineDefinition, RoutineRunRecord,
} from "../api";
import {
  fetchPersonalMemory, fetchAutonomySettings, fetchRoutines, fetchActiveMemory,
} from "../api";

export interface UseAssistantStateOptions {
  activeSessionId: string | null;
  activeProject: number;
  memoryOpen: boolean;
  autonomyOpen: boolean;
  routinesOpen: boolean;
}

export function useAssistantState(opts: UseAssistantStateOptions) {
  const { activeSessionId, activeProject, memoryOpen, autonomyOpen, routinesOpen } = opts;

  // ── State ──
  const [personalMemory, setPersonalMemory] = useState<PersonalMemoryItem[]>([]);
  const [autonomyMode, setAutonomyMode] = useState<AutonomyMode>("observe");
  const [routineCache, setRoutineCache] = useState<RoutineDefinition[]>([]);
  const [routineRunCache, setRoutineRunCache] = useState<RoutineRunRecord[]>([]);
  const [activeMemoryItems, setActiveMemoryItems] = useState<PersonalMemoryItem[]>([]);

  // ── Shared routine refresh helper ──
  const refreshRoutines = useCallback(() => {
    fetchRoutines()
      .then((resp) => {
        setRoutineCache(resp.routines ?? []);
        setRoutineRunCache(resp.runs ?? []);
      })
      .catch(() => {});
  }, []);

  // ── Backend-driven active memory — deferred so it doesn't block session switch rendering ──
  const memoryGenRef = useRef(0);
  useEffect(() => {
    // Clear stale items immediately so old session's memories are never shown or injected
    setActiveMemoryItems([]);
    const gen = ++memoryGenRef.current;
    const load = () => {
      fetchActiveMemory(activeProject, activeSessionId)
        .then((resp) => {
          if (gen !== memoryGenRef.current) return; // stale — discard
          setActiveMemoryItems(Array.isArray(resp?.memory) ? (resp.memory as PersonalMemoryItem[]).filter(Boolean) : []);
        })
        .catch(() => {});
    };
    const id = (typeof requestIdleCallback === "function")
      ? requestIdleCallback(load)
      : setTimeout(load, 0) as unknown as number;
    return () => {
      if (typeof cancelIdleCallback === "function") cancelIdleCallback(id);
      else clearTimeout(id);
    };
  }, [activeProject, activeSessionId]);

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
  }, [routinesOpen, refreshRoutines]);

  // Live routine updates from SSE — refetch routine cache when backend signals a change
  useEffect(() => {
    const handler = () => { refreshRoutines(); };
    window.addEventListener("opman:routine-updated", handler);
    return () => window.removeEventListener("opman:routine-updated", handler);
  }, [refreshRoutines]);

  return {
    personalMemory, setPersonalMemory,
    autonomyMode, setAutonomyMode,
    routineCache, setRoutineCache,
    routineRunCache, setRoutineRunCache,
    activeMemoryItems,
  };
}
