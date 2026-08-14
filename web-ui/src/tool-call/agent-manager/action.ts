import {
  Activity,
  Bot,
  ListTree,
  Octagon,
  Play,
  Send,
  Settings2,
  Hourglass,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

/**
 * Which agent-manager tool a card is rendering.
 *
 * One action per tool the MCP server exposes, so every card is a purpose-built
 * surface rather than the generic JSON dump. `other` is the escape hatch for a
 * name the server grows later — it still renders as a manager card, just
 * without a shape-specific body.
 */
export type AgentManagerAction =
  | "start"
  | "send"
  | "progress"
  | "list"
  | "wait"
  | "abort"
  | "options"
  | "other";

/**
 * Matched on a suffix rather than equality: every MCP host prefixes the name
 * its own way (`mcp__agent-manager__agent_send`, `agent-manager_agent_send`),
 * and the prefix is not ours to predict.
 */
const PATTERNS: readonly (readonly [string, AgentManagerAction])[] = [
  ["agent_runner_options", "options"],
  ["agent_start", "start"],
  ["agent_send", "send"],
  ["agent_progress", "progress"],
  ["agent_list", "list"],
  ["agent_wait", "wait"],
  ["agent_abort", "abort"],
];

export function agentManagerAction(toolName: string): AgentManagerAction {
  const name = toolName.toLowerCase();
  const hit = PATTERNS.find(([needle]) => name.includes(needle));
  return hit ? hit[1] : "other";
}

export function isAgentManagerTool(toolName: string): boolean {
  return agentManagerAction(toolName) !== "other";
}

export interface ActionMeta {
  readonly label: string;
  /** Suffix of the `am-card-*` tone class; drives the head icon colour. */
  readonly tone: string;
  readonly Icon: LucideIcon;
}

export const ACTION_META: Readonly<Record<AgentManagerAction, ActionMeta>> = {
  start: { label: "Start agent", tone: "start", Icon: Play },
  send: { label: "Send to agent", tone: "send", Icon: Send },
  progress: { label: "Agent progress", tone: "progress", Icon: Activity },
  list: { label: "Agent sessions", tone: "list", Icon: ListTree },
  wait: { label: "Wait for agent", tone: "wait", Icon: Hourglass },
  abort: { label: "Abort agent", tone: "abort", Icon: Octagon },
  options: { label: "Runner options", tone: "options", Icon: Settings2 },
  other: { label: "Agent manager", tone: "progress", Icon: Bot },
};
