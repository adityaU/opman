import React from "react";
import { CircleSlash, KeyRound, ShieldCheck, Zap } from "lucide-react";
import type { McpServer } from "../../api/mcp";

/**
 * What a row says about itself.
 *
 * Icon *and* text, never colour alone: "needs login" and "connected" are the two states a
 * user acts on, and a red-green pair is exactly the distinction some people cannot see.
 */

export type Tone = "success" | "warning" | "muted";

export interface Status {
  readonly label: string;
  readonly tone: Tone;
  readonly icon: React.ReactNode;
}

export function statusOf(server: McpServer): Status {
  if (!server.enabled) {
    return { label: "Disabled", tone: "muted", icon: <CircleSlash size={13} /> };
  }
  if (server.auth === "oauth") {
    return server.authenticated
      ? { label: "Connected", tone: "success", icon: <ShieldCheck size={13} /> }
      : { label: "Needs login", tone: "warning", icon: <KeyRound size={13} /> };
  }
  return { label: "Enabled", tone: "success", icon: <Zap size={13} /> };
}

/** Where a server lives, in the fewest words that stay accurate. */
export function originOf(server: McpServer): string {
  if (server.url) return server.url;
  if (server.command) return [server.command, ...server.args].join(" ");
  return server.builtin ? "Built into opman" : "No transport declared";
}
