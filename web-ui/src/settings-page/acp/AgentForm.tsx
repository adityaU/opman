import React, { useCallback, useMemo, useState } from "react";
import type { AcpAgent, AcpAgentDraft } from "../../api/acp";
import { NO_EDITS, SecretEditor, type SecretEdits } from "../mcp/SecretEditor";
import { BLANK_FIELDS, diffFields, fieldsOf, type AgentFields } from "./draft";

/**
 * Declare or edit one ACP agent.
 *
 * The fields are shown resolved — a built-in arrives with opman's own command already in
 * the box — but only what the user moves is written. See [`diffFields`] for why: an entry
 * that restated the built-in would pin it to today's definition.
 */

export interface AgentFormProps {
  /** Absent when declaring a new agent. */
  readonly agent?: AcpAgent;
  readonly saving: boolean;
  readonly onSubmit: (id: string, draft: AcpAgentDraft) => Promise<boolean>;
  readonly onCancel: () => void;
}

/** Ids become runner labels and file names, so they are held to the loader's own shape. */
const ID_SHAPE = /^[a-z0-9][a-z0-9._-]*$/;

export function AgentForm({ agent, saving, onSubmit, onCancel }: AgentFormProps) {
  const base = useMemo(() => (agent ? fieldsOf(agent) : BLANK_FIELDS), [agent]);
  const [id, setId] = useState(agent?.id ?? "");
  const [fields, setFields] = useState<AgentFields>(base);
  const [env, setEnv] = useState<SecretEdits>(NO_EDITS);
  const [problem, setProblem] = useState<string>();

  const set = useCallback(
    <K extends keyof AgentFields>(key: K, value: AgentFields[K]) =>
      setFields((current) => ({ ...current, [key]: value })),
    [],
  );

  const caps = fields.clientCaps;
  const toggleCap = (key: keyof AgentFields["clientCaps"]) =>
    set("clientCaps", { ...caps, [key]: !caps[key] });

  const invalid = useMemo(() => {
    if (!ID_SHAPE.test(id.trim())) {
      return "An id must start with a lower-case letter or digit, and hold only those plus - _ and .";
    }
    if (!fields.command.trim()) return "An agent needs a command to launch.";
    if (fields.runner.trim() && !ID_SHAPE.test(fields.runner.trim())) {
      return "A runner slot has the same shape as an id.";
    }
    return undefined;
  }, [id, fields.command, fields.runner]);

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (invalid) {
        setProblem(invalid);
        return;
      }
      setProblem(undefined);
      const draft = diffFields(base, fields, env);
      // A new agent that only ever needed its command still has to send one, even when the
      // field happens to match the blank baseline it was diffed against.
      if (!agent && draft.command === undefined) draft.command = fields.command.trim();
      if (await onSubmit(id.trim(), draft)) onCancel();
    },
    [invalid, base, fields, env, agent, onSubmit, id, onCancel],
  );

  return (
    <form className="stg-form" onSubmit={submit}>
      <div className="stg-form-grid">
        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-agent-id">
            Id
          </label>
          <input
            id="stg-agent-id"
            className="stg-input"
            value={id}
            readOnly={Boolean(agent)}
            placeholder="gemini"
            spellCheck={false}
            onChange={(event) => setId(event.target.value)}
          />
          <p className="stg-hint">
            The key in <code>acp.json</code>, and the runner slot unless one is set below.
          </p>
        </div>

        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-agent-name">
            Display name
          </label>
          <input
            id="stg-agent-name"
            className="stg-input"
            value={fields.displayName}
            placeholder={id || "Gemini"}
            onChange={(event) => set("displayName", event.target.value)}
          />
          <p className="stg-hint">What the engine picker shows. Blank uses the id.</p>
        </div>
      </div>

      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-agent-command">
          Command
        </label>
        <input
          id="stg-agent-command"
          className="stg-input"
          value={fields.command}
          placeholder="npx"
          spellCheck={false}
          onChange={(event) => set("command", event.target.value)}
        />
      </div>

      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-agent-args">
          Arguments
        </label>
        <textarea
          id="stg-agent-args"
          className="stg-input stg-textarea"
          rows={3}
          value={fields.args}
          placeholder={"-y\n@agentclientprotocol/some-acp@latest"}
          spellCheck={false}
          onChange={(event) => set("args", event.target.value)}
        />
        <p className="stg-hint">One per line, so an argument may contain spaces.</p>
      </div>

      <div className="stg-form-grid">
        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-agent-runner">
            Runner slot
          </label>
          <input
            id="stg-agent-runner"
            className="stg-input"
            value={fields.runner}
            placeholder={id || "gemini"}
            spellCheck={false}
            onChange={(event) => set("runner", event.target.value)}
          />
          <p className="stg-hint">
            Sessions are stored against this. Two agents cannot share one, and{" "}
            <code>opencode</code> and <code>claude-code</code> are already taken.
          </p>
        </div>

        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-agent-mode">
            Opening mode
          </label>
          <input
            id="stg-agent-mode"
            className="stg-input"
            value={fields.defaultMode}
            placeholder="bypassPermissions"
            spellCheck={false}
            onChange={(event) => set("defaultMode", event.target.value)}
          />
          <p className="stg-hint">
            The ACP <code>mode</code> a new session starts in. Blank lets the agent choose.
          </p>
        </div>

        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-agent-model">
            Opening model
          </label>
          <input
            id="stg-agent-model"
            className="stg-input"
            value={fields.defaultModel}
            placeholder="the agent's own default"
            spellCheck={false}
            onChange={(event) => set("defaultModel", event.target.value)}
          />
        </div>
      </div>

      <fieldset className="stg-field">
        <legend className="stg-label">Behaviour</legend>
        <div className="stg-checks">
          <label className="stg-check">
            <input
              type="checkbox"
              checked={fields.injectMcp}
              onChange={() => set("injectMcp", !fields.injectMcp)}
            />
            Offer opman's MCP servers
          </label>
          <label className="stg-check">
            <input
              type="checkbox"
              checked={fields.modesAreAgents}
              onChange={() => set("modesAreAgents", !fields.modesAreAgents)}
            />
            Modes are agents, not permissions
          </label>
          <label className="stg-check">
            <input
              type="checkbox"
              checked={fields.subagentTranscripts}
              onChange={() => set("subagentTranscripts", !fields.subagentTranscripts)}
            />
            Nest subagent sessions
          </label>
        </div>
        <p className="stg-hint">
          Turn <em>modes are agents</em> on when the agent fills ACP's <code>mode</code> slot
          with its own agents rather than permission modes — opencode does, Claude does not.
          Nesting reads Claude-format transcripts, so it only applies to agents that write
          them.
        </p>
      </fieldset>

      <fieldset className="stg-field">
        <legend className="stg-label">Work opman does for the agent</legend>
        <div className="stg-checks">
          <label className="stg-check">
            <input type="checkbox" checked={caps.readTextFile} onChange={() => toggleCap("readTextFile")} />
            Read files
          </label>
          <label className="stg-check">
            <input type="checkbox" checked={caps.writeTextFile} onChange={() => toggleCap("writeTextFile")} />
            Write files
          </label>
          <label className="stg-check">
            <input type="checkbox" checked={caps.terminal} onChange={() => toggleCap("terminal")} />
            Run terminals
          </label>
        </div>
        <p className="stg-hint">
          Leave these off for an agent that brings its own file and terminal tools — which
          most do, and which opman already renders.
        </p>
      </fieldset>

      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-agent-strip">
          Strip from the environment
        </label>
        <textarea
          id="stg-agent-strip"
          className="stg-input stg-textarea"
          rows={2}
          value={fields.envRemove}
          placeholder="MY_VAR"
          spellCheck={false}
          onChange={(event) => set("envRemove", event.target.value)}
        />
        <p className="stg-hint">
          One name per line, unset on the child. opman always strips its own{" "}
          <code>CLAUDECODE</code> markers on top of these.
        </p>
      </div>

      <SecretEditor
        label="Environment"
        existing={agent?.envNames ?? []}
        edits={env}
        onChange={setEnv}
        namePlaceholder="API_KEY"
      />

      {problem && (
        <p className="stg-error" role="alert">
          {problem}
        </p>
      )}

      <div className="stg-form-actions">
        <button type="submit" className="stg-btn is-primary" disabled={saving}>
          {saving ? "Saving…" : agent ? "Save changes" : "Add agent"}
        </button>
        <button type="button" className="stg-btn" onClick={onCancel} disabled={saving}>
          Cancel
        </button>
      </div>
    </form>
  );
}
