import { createContext, useContext } from "react";

/**
 * Opening the session an agent-manager card is talking about.
 *
 * A card knows a session id; it does not know which project owns it, whether
 * the workspace is mounted, or which pane the user wants it in. All three are
 * the shell's answer, so the shell publishes one opener and the cards ask.
 *
 * `open` deliberately does not take a pane: on the desktop it arms the same
 * pane-target overlay the sidebar's session click arms, so "same pane, split,
 * or new window" is one question asked one way everywhere in the app.
 */
export interface AgentSessionOpener {
  /** False when no open project holds the session — the link is not drawn. */
  readonly canOpen: (sessionId: string) => boolean;
  readonly open: (sessionId: string, label: string) => void;
}

const AgentSessionOpenContext = createContext<AgentSessionOpener | null>(null);

export const AgentSessionOpenProvider = AgentSessionOpenContext.Provider;

/** Returns the opener if a provider is mounted, else null. */
export function useAgentSessionOpener(): AgentSessionOpener | null {
  return useContext(AgentSessionOpenContext);
}
