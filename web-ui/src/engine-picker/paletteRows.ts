/**
 * The palette's rows as one flat list, in the order the keyboard walks them.
 *
 * Runner, model and agent are separate groups on screen but one sequence for the arrow
 * keys, so the cursor is a single index and pointer hover writes to the same index —
 * pointer and keyboard can never disagree about what Enter picks.
 */
import type { AgentInfo } from "../api";
import type { ModelOption } from "./useEngineOptions";
import { RUNNER_LABELS } from "../prompt-input/helpers";

export const runnerLabel = (id: string) => RUNNER_LABELS[id] || id;

export type Row =
  | { kind: "runner"; id: string; label: string; selected: boolean }
  | { kind: "model"; id: string; label: string; hint: string; selected: boolean; model: ModelOption }
  | { kind: "agent"; id: string; label: string; hint: string; selected: boolean; agent: AgentInfo };

interface Input {
  query: string;
  runner: string;
  availableRunners: string[];
  models: ModelOption[];
  agents: AgentInfo[];
  selectedModel: { providerID: string; modelID: string } | null;
  selectedAgent: string;
}

export function buildRows(input: Input): Row[] {
  const q = input.query.trim().toLowerCase();
  const match = (...fields: string[]) => !q || fields.some((field) => field.toLowerCase().includes(q));

  const runners: Row[] = input.availableRunners
    .filter((id) => match(id, runnerLabel(id)))
    .map((id) => ({ kind: "runner", id, label: runnerLabel(id), selected: id === input.runner }));

  // The provider name only tells models apart when there is more than one provider; for a
  // single-provider runner it repeats the "Model for Claude" heading on every row.
  const multiProvider = new Set(input.models.map((m) => m.providerId)).size > 1;
  const models: Row[] = input.models
    .filter((m) => match(m.modelName, m.modelId, m.providerName))
    .map((m) => ({
      kind: "model",
      id: `${m.providerId}/${m.modelId}`,
      label: m.modelName,
      hint: multiProvider ? m.providerName : "",
      selected: input.selectedModel?.modelID === m.modelId && input.selectedModel?.providerID === m.providerId,
      model: m,
    }));

  const agents: Row[] = input.agents
    .filter((a) => !a.hidden && match(a.label || a.id, a.id, a.description || ""))
    .map((a) => ({
      kind: "agent",
      id: a.id,
      label: a.label || a.id,
      hint: a.description || "",
      selected: a.id === input.selectedAgent,
      agent: a,
    }));

  return [...runners, ...models, ...agents];
}
