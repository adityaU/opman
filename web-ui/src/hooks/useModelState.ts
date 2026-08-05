import { useState, useMemo, useCallback, useRef } from "react";
import type { ModelRef } from "../api";

export interface ModelState {
  selectedModel: ModelRef | null;
  setSelectedModel: (m: ModelRef | null) => void;
  selectedAgent: string;
  setSelectedAgent: (a: string) => void;
  sending: boolean;
  /** Set sending state. Pass sessionId to target a specific session (used by finally blocks). */
  setSending: (v: boolean, sessionId?: string) => void;
  currentModel: string | null;
  defaultModelDisplay: string | null;
  currentModelContextLimit: number | null;
}

export function useModelState(
  messages: any[],
  providers: { defaults: Record<string, string>; all: any[] },
  activeSessionId: string | null,
): ModelState {
  const [selectedModel, setSelectedModel] = useState<ModelRef | null>(null);
  const [selectedAgent, setSelectedAgent] = useState("");

  // Per-session sending state: Map<sessionId, boolean>.
  // The exposed `sending` boolean is derived from the active session.
  const sendingMap = useRef(new Map<string, boolean>());
  const [sendingFlag, setSendingFlag] = useState(false);

  // A send that creates its own session starts while activeSessionId is still
  // null and finishes once it is set. Read the id through a ref so the
  // completion path compares against the live value rather than the one
  // captured when the send began — otherwise the flag never flips back and the
  // in-flight indicator sticks until an unrelated re-render.
  const activeSessionIdRef = useRef(activeSessionId);
  activeSessionIdRef.current = activeSessionId;

  const setSending = useCallback((v: boolean, sessionId?: string) => {
    const sid = sessionId ?? activeSessionIdRef.current;
    if (sid) {
      if (v) sendingMap.current.set(sid, true);
      else sendingMap.current.delete(sid);
    }
    // Only update the React flag if the target session is the currently active one
    if (!sid || sid === activeSessionIdRef.current) setSendingFlag(v);
  }, []);

  // When activeSessionId changes, derive sending from the map
  const sending = activeSessionId
    ? (sendingMap.current.get(activeSessionId) ?? false)
    : sendingFlag;

  // Derive current model from selectedModel or latest assistant message
  const currentModel = useMemo(() => {
    if (selectedModel) return selectedModel.modelID;
    for (let i = messages.length - 1; i >= 0; i--) {
      const msg = messages[i];
      if (msg.info.role === "assistant") {
        if (msg.info.modelID) return msg.info.modelID;
        if (msg.info.model) {
          if (typeof msg.info.model === "string") return msg.info.model;
          return msg.info.model.modelID || null;
        }
      }
    }
    return null;
  }, [selectedModel, messages]);

  // Derive default model for new session display (from provider defaults)
  const defaultModelDisplay = useMemo(() => {
    if (currentModel) return currentModel;
    if (selectedModel) return selectedModel.modelID;
    const defaultEntries = Object.entries(providers.defaults);
    if (defaultEntries.length > 0) {
      return defaultEntries[0][1];
    }
    return null;
  }, [currentModel, selectedModel, providers.defaults]);

  // Derive context limit for the current model from providers
  const currentModelContextLimit = useMemo(() => {
    const modelId = currentModel || defaultModelDisplay;
    if (!modelId || !providers.all.length) return null;
    for (const provider of providers.all) {
      for (const [, model] of Object.entries(provider.models) as [string, any][]) {
        if (model.id === modelId && model.limit?.context) {
          return model.limit.context;
        }
      }
    }
    return null;
  }, [currentModel, defaultModelDisplay, providers.all]);

  return {
    selectedModel,
    setSelectedModel,
    selectedAgent,
    setSelectedAgent,
    sending,
    setSending,
    currentModel,
    defaultModelDisplay,
    currentModelContextLimit,
  };
}
