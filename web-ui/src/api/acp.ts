import { apiFetch, apiPost, apiPut } from "./client";

/**
 * `~/.config/opman/acp.json`, read and written through the backend.
 *
 * An ACP agent is a *runner*, not a tool: declaring one here is what puts an engine in the
 * picker. Every write reconciles the live engines against the file and broadcasts
 * `acp_agents_changed`, so an agent added on this page can be selected in the next message
 * without restarting opman.
 *
 * Field names match the config document's own spelling — this module is an editor for that
 * file, not a separate representation of it.
 */

/** Window event re-broadcast from the `acp_agents_changed` SSE event. */
export const ACP_AGENTS_CHANGED = "opman:acp-agents-changed";

/** What opman offers to do on the agent's behalf. Off means the agent uses its own tools. */
export interface AcpClientCaps {
  readTextFile: boolean;
  writeTextFile: boolean;
  terminal: boolean;
}

export const NO_CLIENT_CAPS: AcpClientCaps = {
  readTextFile: false,
  writeTextFile: false,
  terminal: false,
};

export interface AcpAgent {
  /** Config key. Also the provider id and the default runner slot. */
  id: string;
  displayName: string;
  command: string;
  args: string[];
  /** Names only — an `env` value is a credential, so it never leaves opman. */
  envNames: string[];
  /** Inherited variables stripped from the child, on top of opman's own list. */
  envRemove: string[];
  runner: string;
  clientCaps: AcpClientCaps;
  injectMcp: boolean;
  defaultMode: string;
  defaultModel: string;
  /** The ACP `mode` slot holds the agent's *agents*, not permission modes. */
  modesAreAgents: boolean;
  subagentTranscripts: boolean;
  enabled: boolean;
  /** opman ships this one. Removing its entry restores the default rather than deleting it. */
  builtin: boolean;
  /**
   * Where opman read this agent's launch command from.
   *
   * Empty for an agent the user declared. For a catalogued one whose upstream docs never
   * state the command, opman ships the row with no command at all — this link is then the
   * only thing the row can offer, and filling the command in is what turns it on.
   */
  docs: string;
  /** The user's file has an entry for it. */
  customized: boolean;
  /** An engine is running and the runner slot is served. */
  running: boolean;
  /** Enabled, with something to launch. */
  launchable: boolean;
  /** Its runner slot belongs to an engine that is not an ACP agent. */
  slotTaken: boolean;
  /**
   * It serves opman's default runner, so an edit lands on the next start rather than
   * immediately — that engine's address went to the TUI once, at startup.
   */
  isDefault: boolean;
}

/**
 * An agent as the settings page submits it — a patch, not a replacement.
 *
 * An omitted field is left as the file has it; an empty string or array is a decision and
 * clears the value. `env` is the exception: its values are never sent to the browser, so it
 * is edited by name through `envSet`/`envUnset` rather than resent whole.
 */
export interface AcpAgentDraft {
  displayName?: string;
  command?: string;
  args?: string[];
  runner?: string;
  clientCaps?: AcpClientCaps;
  injectMcp?: boolean;
  defaultMode?: string;
  defaultModel?: string;
  modesAreAgents?: boolean;
  subagentTranscripts?: boolean;
  enabled?: boolean;
  envRemove?: string[];
  envSet?: Record<string, string>;
  envUnset?: string[];
}

/** What a write actually did to the running set. */
export interface AcpWriteResult {
  status: string;
  /** Runner slots that started serving. */
  started: string[];
  stopped: string[];
  /** Agent ids whose runner slot is held by something else, so they did not start. */
  blocked: string[];
  /** Agent ids saved to disk but still running their old definition. */
  deferred: string[];
}

const agent = (id: string) => `/acp/agents/${encodeURIComponent(id)}`;

/** Every agent opman knows about: the built-ins and anything declared in config. */
export async function fetchAcpAgents(): Promise<AcpAgent[]> {
  const raw = await apiFetch<Partial<AcpAgent>[]>("/acp/agents");
  // Every list and object field is defaulted on the way in: the rows render them directly,
  // so one absent key from an older backend would take the section down rather than degrade.
  return raw.map((entry) => ({
    displayName: entry.id ?? "",
    command: "",
    runner: entry.id ?? "",
    injectMcp: true,
    defaultMode: "",
    defaultModel: "",
    modesAreAgents: false,
    subagentTranscripts: false,
    enabled: true,
    builtin: false,
    docs: "",
    customized: false,
    running: false,
    launchable: false,
    slotTaken: false,
    isDefault: false,
    ...entry,
    id: entry.id ?? "",
    args: entry.args ?? [],
    envNames: entry.envNames ?? [],
    envRemove: entry.envRemove ?? [],
    clientCaps: { ...NO_CLIENT_CAPS, ...entry.clientCaps },
  }));
}

export async function saveAcpAgent(id: string, draft: AcpAgentDraft): Promise<AcpWriteResult> {
  return apiPut<AcpWriteResult>(agent(id), draft);
}

/**
 * Turn an agent on or off.
 *
 * Works for a built-in with no entry of its own: the backend records a one-field patch, so
 * opman's launch command is never copied into user config where it would then go stale.
 */
export async function setAcpAgentEnabled(id: string, enabled: boolean): Promise<AcpWriteResult> {
  return apiPost<AcpWriteResult>(`${agent(id)}/enabled`, { enabled });
}

/**
 * Drop the user's entry. For a built-in that restores opman's own definition.
 *
 * Read through `apiFetch` rather than the `apiDelete` helper because the reply matters:
 * which runners stopped is what the page reports back, and a delete that returns nothing
 * could not say it.
 */
export async function deleteAcpAgent(id: string): Promise<AcpWriteResult> {
  return apiFetch<AcpWriteResult>(agent(id), { method: "DELETE" });
}

/**
 * Delete `acp.json` and put every agent back to how opman ships it.
 *
 * The per-agent Remove drops one entry; this drops the file. Both mean "stop overriding",
 * at the two scopes the file actually has — and the whole-file one is the only way to undo
 * a config that has gone wrong without first working out what is in it.
 */
export async function resetAcpConfig(): Promise<AcpWriteResult> {
  return apiFetch<AcpWriteResult>("/acp/agents", { method: "DELETE" });
}
