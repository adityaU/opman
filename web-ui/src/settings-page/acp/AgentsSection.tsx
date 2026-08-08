import React, { useCallback, useState } from "react";
import { Plus, X } from "lucide-react";
import type { AcpAgent } from "../../api/acp";
import { AgentForm } from "./AgentForm";
import { AgentRow } from "./AgentRow";
import { useAcpAgents } from "./useAcpAgents";

/**
 * ACP agents: the engines opman can drive.
 *
 * This is the one settings section that adds a *runner* rather than something a runner
 * uses. An agent declared here appears in the engine picker as soon as it starts, without
 * restarting opman — so the list has to say whether each one is actually running, not only
 * whether it is configured.
 */

export interface AgentsSectionProps {
  readonly onError: (message: string) => void;
}

type Editing = { readonly kind: "none" } | { readonly kind: "new" } | { readonly kind: "agent"; readonly id: string };

export function AgentsSection({ onError }: AgentsSectionProps) {
  const state = useAcpAgents(onError);
  const [editing, setEditing] = useState<Editing>({ kind: "none" });
  const [confirming, setConfirming] = useState<string>();

  const close = useCallback(() => setEditing({ kind: "none" }), []);

  const remove = useCallback(
    async (id: string) => {
      setConfirming(undefined);
      await state.remove(id);
      setEditing((current) => (current.kind === "agent" && current.id === id ? { kind: "none" } : current));
    },
    [state],
  );

  const editingAgent = (agent: AcpAgent) => editing.kind === "agent" && editing.id === agent.id;

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <div className="stg-card-head">
          <div>
            <h3 className="stg-card-title">Declared agents</h3>
            <p className="stg-card-note">
              Any server speaking the Agent Client Protocol can be an engine here — opman
              ships Claude and Codex and speaks only ACP to both. Adding one takes effect
              immediately: it becomes a runner you can start a session in, and removing one
              stops its processes.
            </p>
          </div>
          <button
            type="button"
            className="stg-btn is-primary"
            onClick={() => setEditing(editing.kind === "new" ? { kind: "none" } : { kind: "new" })}
            aria-expanded={editing.kind === "new"}
          >
            <Plus size={13} aria-hidden="true" />
            Add agent
          </button>
        </div>

        {editing.kind === "new" && (
          <AgentForm saving={state.busy !== undefined} onSubmit={state.save} onCancel={close} />
        )}

        {state.notice && (
          <p className="stg-note" role="status">
            {state.notice}
            <button
              type="button"
              className="stg-icon-btn"
              onClick={state.dismissNotice}
              aria-label="Dismiss"
            >
              <X size={13} />
            </button>
          </p>
        )}

        {state.error && (
          <p className="stg-error" role="alert">
            {state.error}
          </p>
        )}

        {state.loading ? (
          <p className="stg-hint">Loading…</p>
        ) : state.agents.length === 0 ? (
          <div className="stg-empty">
            <p>No ACP agents.</p>
            <p className="stg-hint">
              Even opman's built-ins are config entries, so an empty list means the config
              file has disabled them all.
            </p>
          </div>
        ) : (
          <ul className="stg-rows">
            {state.agents.map((agent) => (
              <React.Fragment key={agent.id}>
                <AgentRow
                  agent={agent}
                  busy={state.busy === agent.id}
                  editing={editingAgent(agent)}
                  onToggle={() => state.toggle(agent)}
                  onEdit={() =>
                    setEditing(editingAgent(agent) ? { kind: "none" } : { kind: "agent", id: agent.id })
                  }
                  onRemove={() => setConfirming(agent.id)}
                />
                {confirming === agent.id && (
                  <li className="stg-confirm">
                    <span>
                      {agent.builtin ? (
                        <>
                          Discard your changes to <strong>{agent.displayName}</strong> and
                          restore opman's own definition?
                        </>
                      ) : (
                        <>
                          Remove <strong>{agent.displayName}</strong> from{" "}
                          <code>acp.json</code>? Its processes are killed and the runner
                          disappears. Sessions already in it stay on disk.
                        </>
                      )}
                    </span>
                    <span className="stg-confirm-actions">
                      <button
                        type="button"
                        className={agent.builtin ? "stg-btn is-primary" : "stg-btn is-danger"}
                        onClick={() => remove(agent.id)}
                      >
                        {agent.builtin ? "Restore" : "Remove"}
                      </button>
                      <button type="button" className="stg-btn" onClick={() => setConfirming(undefined)}>
                        Keep
                      </button>
                    </span>
                  </li>
                )}
                {editingAgent(agent) && (
                  <li className="stg-row-form">
                    <AgentForm
                      agent={agent}
                      saving={state.busy === agent.id}
                      onSubmit={state.save}
                      onCancel={close}
                    />
                  </li>
                )}
              </React.Fragment>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
