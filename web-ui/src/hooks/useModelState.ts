import { useState, useMemo } from "react";
import type { ModelRef } from "../api";

export interface ModelState {
  selectedModel: ModelRef | null;
  setSelectedModel: (m: ModelRef | null) => void;
  selectedAgent: string;
  setSelectedAgent: (a: string) => void;
  sending: boolean;
  setSending: (v: boolean) => void;
  currentModel: string | null;
  defaultModelDisplay: string | null;
  currentModelContextLimit: number | null;
}

export function useModelState(
  messages: any[],
  providers: { defaults: Record<string, string>; all: any[] },
): ModelState {
  const [selectedModel, setSelectedModel] = useState<ModelRef | null>(null);
  const [selectedAgent, setSelectedAgent] = useState("");
  const [sending, setSending] = useState(false);

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
