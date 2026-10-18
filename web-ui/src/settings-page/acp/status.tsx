import React from "react";
import { AlertTriangle, CircleSlash, Play, TriangleAlert } from "lucide-react";
import type { AcpAgent } from "../../api/acp";
import type { AuthEntry } from "./useAcpAuth";

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

/** Connected or not connected, and what is known about why. */
export interface AuthStatus {
  readonly label: string;
  readonly tone: "success" | "muted" | "warning";
  readonly checking: boolean;
  readonly connected: boolean;
  readonly detail: string;
  /** The shell command that logs the agent in, when one can. */
  readonly command: string | null;
}

export function authStatusOf(entry: AuthEntry): AuthStatus {
  switch (entry.status) {
    case "checking":
      return { label: "Checking…", tone: "muted", checking: true, connected: false, detail: "", command: null };
    case "failed":
      return { label: "Not connected", tone: "warning", checking: false, connected: false, detail: entry.message, command: null };
    case "known":
      return entry.auth.state === "connected"
        ? { label: "Connected", tone: "success", checking: false, connected: true, detail: "", command: null }
        : {
            label: "Not connected",
            tone: "warning",
            checking: false,
            connected: false,
            detail: entry.auth.detail,
            command: entry.auth.loginCommand,
          };
  }
}
