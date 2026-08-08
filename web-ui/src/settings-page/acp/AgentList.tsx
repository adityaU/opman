import React from "react";
import type { AcpAgent } from "../../api/acp";
import { AgentForm } from "./AgentForm";
import { AgentRow } from "./AgentRow";
import type { AcpAgentsState } from "./useAcpAgents";

/**
 * One list of agents, with the confirm and the edit form that belong to a row.
 *
 * Split from the section because there are now two lists — the agents that are on, and the
 * catalogue of harnesses that are merely known — and the row, its delete confirmation and
 * its inline form are the same three things in both.
 */

export interface AgentListProps {
  readonly agents: readonly AcpAgent[];
  readonly state: AcpAgentsState;
  /** Id whose form is open, if any. */
  readonly editing: string | undefined;
  readonly onEdit: (id: string) => void;
  readonly onCloseForm: () => void;
  /** Id whose removal is awaiting confirmation, if any. */
  readonly confirming: string | undefined;
  readonly onConfirm: (id: string | undefined) => void;
  readonly onRemove: (id: string) => void;
}

export function AgentList({
  agents,
  state,
  editing,
  onEdit,
  onCloseForm,
  confirming,
  onConfirm,
  onRemove,
}: AgentListProps) {
  return (
    <ul className="stg-rows">
      {agents.map((agent) => (
        <React.Fragment key={agent.id}>
          <AgentRow
            agent={agent}
            busy={state.busy === agent.id}
            editing={editing === agent.id}
            onToggle={() => state.toggle(agent)}
            onEdit={() => onEdit(agent.id)}
            onRemove={() => onConfirm(agent.id)}
          />
          {confirming === agent.id && (
            <li className="stg-confirm">
              <span>
                {agent.builtin ? (
                  <>
                    Discard your changes to <strong>{agent.displayName}</strong> and restore
                    opman's own definition?
                  </>
                ) : (
                  <>
                    Remove <strong>{agent.displayName}</strong> from <code>acp.json</code>? Its
                    processes are killed and the runner disappears. Sessions already in it stay
                    on disk.
                  </>
                )}
              </span>
              <span className="stg-confirm-actions">
                <button
                  type="button"
                  className={agent.builtin ? "stg-btn is-primary" : "stg-btn is-danger"}
                  onClick={() => onRemove(agent.id)}
                >
                  {agent.builtin ? "Restore" : "Remove"}
                </button>
                <button type="button" className="stg-btn" onClick={() => onConfirm(undefined)}>
                  Keep
                </button>
              </span>
            </li>
          )}
          {editing === agent.id && (
            <li className="stg-row-form">
              <AgentForm
                agent={agent}
                saving={state.busy === agent.id}
                onSubmit={state.save}
                onCancel={onCloseForm}
              />
            </li>
          )}
        </React.Fragment>
      ))}
    </ul>
  );
}
