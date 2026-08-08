import React from "react";
import { AlertTriangle, CircleSlash, Play, TriangleAlert } from "lucide-react";
import type { AcpAgent } from "../../api/acp";

/**
 * The one question a row has to answer at a glance: is this agent usable right now.
 *
 * Four states, and the distinction that matters is between the two failure ones. "Off" is
 * the user's own doing. "Not running" is opman's — the agent is configured and enabled and
 * still is not there, which is a thing to go and fix.
 */

export interface AgentStatus {
  readonly label: string;
  readonly tone: "success" | "muted" | "warning";
  readonly icon: React.ReactNode;
}

export function statusOf(agent: AcpAgent): AgentStatus {
  if (agent.running) {
    return { label: "Running", tone: "success", icon: <Play size={11} aria-hidden="true" /> };
  }
  if (!agent.enabled) {
    return { label: "Off", tone: "muted", icon: <CircleSlash size={11} aria-hidden="true" /> };
  }
  if (agent.slotTaken) {
    return {
      label: "Slot taken",
      tone: "warning",
      icon: <TriangleAlert size={11} aria-hidden="true" />,
    };
  }
  return {
    label: agent.command ? "Not running" : "No command",
    tone: "warning",
    icon: <AlertTriangle size={11} aria-hidden="true" />,
  };
}

/** The command line as it will be spawned, for the row's second line. */
export function originOf(agent: AcpAgent): string {
  return [agent.command, ...agent.args].filter(Boolean).join(" ") || "No launch command";
}
