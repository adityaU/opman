/**
 * Options for the engine palette, and the repair that keeps them consistent.
 *
 * A runner owns its models and its agents. Selecting one therefore invalidates
 * the other two, and the previous split controls only ever half-admitted that:
 * the agent list refetched per runner while the *selected* agent was left
 * alone, and the model was never checked at all — so a session could sit there
 * claiming a model its runner has never heard of. Ownership of that repair
 * belongs in one place, next to the lists it repairs.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import type { AgentInfo } from "../api";
import type { PermissionModeOption } from "../api/session";
import { fetchAgents } from "../api";
import { useProviders } from "../hooks/useProviders";

export interface ModelOption {
  providerId: string;
  providerName: string;
  modelId: string;
  modelName: string;
  isDefault: boolean;
  isConnected: boolean;
}

export interface EngineOptions {
  models: ModelOption[];
  agents: AgentInfo[];
  /**
   * Permission modes the engine reported, or null when it reports none and the caller
   * should fall back to its own table. ACP agents come from config, so their modes can
   * only be discovered — never looked up by runner name.
   */
  permissionModes: PermissionModeOption[] | null;
  loading: boolean;
}

export function useEngineOptions(
  runner: string,
  selectedModel: { providerID: string; modelID: string } | null,
  selectedAgent: string,
  onModelSelected: (modelId: string, providerId: string) => void,
  onAgentChange: (agentId: string) => void,
): EngineOptions {
  const providers = useProviders(runner);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loadingAgents, setLoadingAgents] = useState(false);

  const models = useMemo<ModelOption[]>(() => {
    const out: ModelOption[] = [];
    for (const provider of providers.all) {
      if (!provider.models) continue;
      for (const [modelId, info] of Object.entries(provider.models)) {
        out.push({
          providerId: provider.id,
          providerName: provider.name || provider.id,
          modelId,
          modelName: info.name || modelId,
          isDefault: providers.defaults[provider.id] === modelId,
          isConnected: providers.connected.has(provider.id),
        });
      }
    }
    out.sort((a, b) => {
      if (a.isConnected !== b.isConnected) return a.isConnected ? -1 : 1;
      if (a.isDefault !== b.isDefault) return a.isDefault ? -1 : 1;
      return a.modelName.localeCompare(b.modelName);
    });
    return out;
  }, [providers.all, providers.defaults, providers.connected]);

  useEffect(() => {
    let cancelled = false;
    setLoadingAgents(true);
    fetchAgents(runner)
      .then((fetched) => {
        if (cancelled) return;
        setAgents(fetched);
      })
      .catch(() => { if (!cancelled) setAgents([]); })
      .finally(() => { if (!cancelled) setLoadingAgents(false); });
    return () => { cancelled = true; };
  }, [runner]);

  // Repair a selection the new runner cannot honour. Only ever downgrades to
  // that runner's own default — never silently swaps one working choice for
  // another, which would hide the runner change from the user.
  const repairedFor = useRef<string>("");
  useEffect(() => {
    if (models.length === 0) return;
    const stillOffered = selectedModel
      && models.some((m) => m.modelId === selectedModel.modelID && m.providerId === selectedModel.providerID);
    if (stillOffered) {
      repairedFor.current = runner;
      return;
    }
    if (repairedFor.current === runner && !selectedModel) return;
    repairedFor.current = runner;
    const fallback = models.find((m) => m.isDefault && m.isConnected) || models[0];
    if (fallback) onModelSelected(fallback.modelId, fallback.providerId);
  }, [models, runner, selectedModel, onModelSelected]);

  useEffect(() => {
    if (agents.length === 0) return;
    if (selectedAgent && agents.some((a) => a.id === selectedAgent)) return;
    onAgentChange(agents[0].id);
  }, [agents, selectedAgent, onAgentChange]);

  return {
    models,
    agents,
    permissionModes: providers.permissionModes,
    loading: providers.loading || loadingAgents,
  };
}
