import type { AcpAgent, AcpAgentDraft, AcpClientCaps } from "../../api/acp";
import { NO_CLIENT_CAPS } from "../../api/acp";
import type { SecretEdits } from "../mcp/SecretEditor";

/**
 * Turning what the form holds into what gets written.
 *
 * The form is shown *resolved* values — the built-in's command, the built-in's mode — but
 * must not write them back. An entry that restated opman's own definition would pin the
 * agent to today's version of it and quietly stop tracking updates. So only fields the user
 * actually moved are submitted, which is exactly the distinction the patch document exists
 * to record.
 */

/** Everything the form edits, before it is diffed into a patch. */
export interface AgentFields {
  readonly displayName: string;
  readonly command: string;
  readonly args: string;
  readonly runner: string;
  readonly defaultMode: string;
  readonly defaultModel: string;
  readonly injectMcp: boolean;
  readonly modesAreAgents: boolean;
  readonly subagentTranscripts: boolean;
  readonly clientCaps: AcpClientCaps;
  readonly envRemove: string;
}

/** What a brand-new agent starts from. `injectMcp` is on, as the config loader defaults it. */
export const BLANK_FIELDS: AgentFields = {
  displayName: "",
  command: "",
  args: "",
  runner: "",
  defaultMode: "",
  defaultModel: "",
  injectMcp: true,
  modesAreAgents: false,
  subagentTranscripts: false,
  clientCaps: NO_CLIENT_CAPS,
  envRemove: "",
};

/** One entry per line, so a value may contain spaces. */
export function toLines(values: readonly string[]): string {
  return values.join("\n");
}

function fromLines(text: string): string[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}

export function fieldsOf(agent: AcpAgent): AgentFields {
  return {
    displayName: agent.displayName,
    command: agent.command,
    args: toLines(agent.args),
    runner: agent.runner,
    defaultMode: agent.defaultMode,
    defaultModel: agent.defaultModel,
    injectMcp: agent.injectMcp,
    modesAreAgents: agent.modesAreAgents,
    subagentTranscripts: agent.subagentTranscripts,
    clientCaps: agent.clientCaps,
    envRemove: toLines(agent.envRemove),
  };
}

function sameCaps(a: AcpClientCaps, b: AcpClientCaps): boolean {
  return (
    a.readTextFile === b.readTextFile &&
    a.writeTextFile === b.writeTextFile &&
    a.terminal === b.terminal
  );
}

/**
 * The patch to submit: what `next` says and `base` did not.
 *
 * `base` is the agent as it was shown, so for an edit this is "what the user changed" and
 * for a new agent it is "everything that is not still blank".
 */
export function diffFields(base: AgentFields, next: AgentFields, env: SecretEdits): AcpAgentDraft {
  const draft: AcpAgentDraft = {};
  if (next.displayName !== base.displayName) draft.displayName = next.displayName.trim();
  if (next.command !== base.command) draft.command = next.command.trim();
  if (next.args !== base.args) draft.args = fromLines(next.args);
  if (next.runner !== base.runner) draft.runner = next.runner.trim();
  if (next.defaultMode !== base.defaultMode) draft.defaultMode = next.defaultMode.trim();
  if (next.defaultModel !== base.defaultModel) draft.defaultModel = next.defaultModel.trim();
  if (next.injectMcp !== base.injectMcp) draft.injectMcp = next.injectMcp;
  if (next.modesAreAgents !== base.modesAreAgents) draft.modesAreAgents = next.modesAreAgents;
  if (next.subagentTranscripts !== base.subagentTranscripts) {
    draft.subagentTranscripts = next.subagentTranscripts;
  }
  if (!sameCaps(next.clientCaps, base.clientCaps)) draft.clientCaps = next.clientCaps;
  if (next.envRemove !== base.envRemove) draft.envRemove = fromLines(next.envRemove);
  if (Object.keys(env.set).length > 0) draft.envSet = env.set;
  if (env.remove.length > 0) draft.envUnset = [...env.remove];
  return draft;
}
