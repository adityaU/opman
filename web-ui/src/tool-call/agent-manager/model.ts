import { asArr, asObj, str } from "../tcUtils";
import type { AgentManagerAction } from "./action";

/**
 * Reading the manager's replies.
 *
 * The bridge hands every result back as pretty-printed JSON text, so each
 * accessor here is defensive by construction: a field that is missing reads as
 * empty rather than throwing, and the card decides what to draw from that.
 * Kept apart from the components so the shapes are testable without a DOM.
 */

export function parseAgentOutput(output: unknown): Record<string, unknown> {
  return asObj(output);
}

export function shortId(value: string): string {
  if (!value) return "";
  return value.length > 18 ? `${value.slice(0, 10)}…${value.slice(-5)}` : value;
}

export function asText(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

export function messageText(message: unknown): string {
  const value = asObj(message);
  const direct = str(value.text);
  if (direct) return direct;
  return asArr(value.parts)
    .map((part) => asText(asObj(part).text))
    .filter(Boolean)
    .join("\n");
}

export function messageRole(message: unknown): string {
  const role = str(asObj(asObj(message).info).role);
  return role === "assistant" || role === "user" ? role : "system";
}

/** Who a call was addressed to. `to`/`agent_id` are the two spellings the schema uses. */
export function displayTarget(input: Record<string, unknown>): string {
  return str(input.to) || str(input.agent_id) || "parent";
}

export function deliveryLabel(value: string): string {
  if (value === "queued" || value === "next_turn" || value === "next-turn") return "next turn";
  return value === "none" ? "created" : "immediate";
}

/** The session a card can hand to a pane, or "" when the reply names none. */
export function openableSessionId(
  action: AgentManagerAction,
  input: Record<string, unknown>,
  output: Record<string, unknown>,
): string {
  const id = str(output.session_id) || str(output.agent_id) || str(input.to) || str(input.agent_id);
  return id === "parent" ? "" : id;
}

// ── agent_list ──────────────────────────────────────────

export interface AgentRow {
  readonly id: string;
  readonly title: string;
  readonly runner: string;
  readonly busy: boolean;
  readonly queued: number;
}

export function agentRows(output: Record<string, unknown>): readonly AgentRow[] {
  return asArr(output.agents).map((entry) => {
    const agent = asObj(entry);
    return {
      id: str(agent.agent_id),
      title: str(agent.title) || shortId(str(agent.agent_id)),
      runner: str(agent.runner),
      busy: agent.busy === true,
      queued: typeof agent.queued_messages === "number" ? agent.queued_messages : 0,
    };
  });
}

// ── agent_runner_options ────────────────────────────────

export interface ModelRow {
  readonly provider: string;
  readonly id: string;
  readonly name: string;
  readonly efforts: readonly string[];
}

export interface RunnerOptions {
  readonly runner: string;
  readonly models: readonly ModelRow[];
  readonly efforts: readonly string[];
  readonly total: number;
  readonly omitted: number;
  readonly connected: readonly string[];
  readonly permissionModes: readonly string[];
  readonly agents: readonly string[];
}

export function runnerOptions(output: Record<string, unknown>): RunnerOptions {
  return {
    runner: str(output.runner),
    models: asArr(output.models).map((entry) => {
      const model = asObj(entry);
      const id = str(model.id);
      return {
        provider: str(model.provider),
        id,
        name: str(model.name) || id,
        efforts: asArr(model.efforts).map(asText).filter(Boolean),
      };
    }),
    efforts: asArr(output.efforts).map(asText).filter(Boolean),
    total: typeof output.total_models === "number" ? output.total_models : 0,
    omitted: typeof output.omitted_models === "number" ? output.omitted_models : 0,
    connected: asArr(output.connected).map(asText).filter(Boolean),
    permissionModes: asArr(output.permission_modes).map(asText).filter(Boolean),
    agents: asArr(output.agents)
      .map((entry) => asText(entry) || str(asObj(entry).name))
      .filter(Boolean),
  };
}
