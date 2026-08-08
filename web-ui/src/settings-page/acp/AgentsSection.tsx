import React, { useCallback, useMemo, useState } from "react";
import { Plus, Trash2, X } from "lucide-react";
import { AgentForm } from "./AgentForm";
import { AgentList } from "./AgentList";
import { useAcpAgents, WHOLE_CONFIG } from "./useAcpAgents";

/**
 * ACP agents: the engines opman can drive.
 *
 * This is the one settings section that adds a *runner* rather than something a runner
 * uses. An agent declared here appears in the engine picker as soon as it starts, without
 * restarting opman — so the list has to say whether each one is actually running, not only
 * whether it is configured.
 *
 * opman now ships the protocol's whole published agent list, which is why the section is in
 * two parts. The top list is what opman is actually driving; the catalogue below it is the
 * rest, declared but off, folded away so thirty rows of "not running" do not read as thirty
 * problems. Turning one on is the same toggle either way.
 */

export interface AgentsSectionProps {
  readonly onError: (message: string) => void;
}

type Editing = { readonly kind: "none" } | { readonly kind: "new" } | { readonly kind: "agent"; readonly id: string };

export function AgentsSection({ onError }: AgentsSectionProps) {
  const state = useAcpAgents(onError);
  const [editing, setEditing] = useState<Editing>({ kind: "none" });
  const [confirming, setConfirming] = useState<string>();
  const [resetting, setResetting] = useState(false);

  const close = useCallback(() => setEditing({ kind: "none" }), []);
  const edit = useCallback(
    (id: string) =>
      setEditing((current) =>
        current.kind === "agent" && current.id === id ? { kind: "none" } : { kind: "agent", id },
      ),
    [],
  );

  const remove = useCallback(
    async (id: string) => {
      setConfirming(undefined);
      await state.remove(id);
      setEditing((current) => (current.kind === "agent" && current.id === id ? { kind: "none" } : current));
    },
    [state],
  );

  const reset = useCallback(async () => {
    setResetting(false);
    setConfirming(undefined);
    setEditing({ kind: "none" });
    await state.resetConfig();
  }, [state]);

  // An agent is either something opman is driving or something it merely knows about, and
  // the two want different prominence. `customized` is also what says the file has anything
  // in it — with no entries there is nothing for a reset to undo.
  const { live, catalogue, customized } = useMemo(
    () => ({
      live: state.agents.filter((agent) => agent.enabled),
      catalogue: state.agents.filter((agent) => !agent.enabled),
      customized: state.agents.some((agent) => agent.customized),
    }),
    [state.agents],
  );

  const openId = editing.kind === "agent" ? editing.id : undefined;
  const listProps = {
    state,
    editing: openId,
    onEdit: edit,
    onCloseForm: close,
    confirming,
    onConfirm: setConfirming,
    onRemove: remove,
  };

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <div className="stg-card-head">
          <div>
            <h3 className="stg-card-title">Declared agents</h3>
            <p className="stg-card-note">
              Any server speaking the Agent Client Protocol can be an engine here — opman
              ships every agent the protocol publishes and speaks only ACP to all of them.
              Enabling one takes effect immediately: it becomes a runner you can start a
              session in or hand an open session over to, and disabling one stops its
              processes.
            </p>
          </div>
          <div className="stg-card-actions">
            <button
              type="button"
              className="stg-btn is-primary"
              onClick={() => setEditing(editing.kind === "new" ? { kind: "none" } : { kind: "new" })}
              aria-expanded={editing.kind === "new"}
            >
              <Plus size={13} aria-hidden="true" />
              Add agent
            </button>
            <button
              type="button"
              className="stg-btn is-danger"
              onClick={() => setResetting((open) => !open)}
              disabled={!customized || state.busy === WHOLE_CONFIG}
              aria-expanded={resetting}
              title={
                customized
                  ? "Delete acp.json and restore every agent to how opman ships it"
                  : "Nothing to reset — acp.json overrides nothing"
              }
            >
              <Trash2 size={13} aria-hidden="true" />
              Reset config
            </button>
          </div>
        </div>

        {resetting && (
          <p className="stg-confirm">
            <span>
              Delete <code>acp.json</code>? Every agent goes back to how opman ships it —
              your commands, environment and enabled/disabled choices are all discarded, and
              agents you declared yourself disappear along with their runners. Sessions stay
              on disk.
            </span>
            <span className="stg-confirm-actions">
              <button type="button" className="stg-btn is-danger" onClick={reset}>
                Delete config
              </button>
              <button type="button" className="stg-btn" onClick={() => setResetting(false)}>
                Keep
              </button>
            </span>
          </p>
        )}

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
        ) : (
          <>
            {live.length === 0 ? (
              <div className="stg-empty">
                <p>No agents enabled.</p>
                <p className="stg-hint">
                  Even opman's own two are config entries, so an empty list means they have
                  been turned off. Enable one from the catalogue below.
                </p>
              </div>
            ) : (
              <AgentList agents={live} {...listProps} />
            )}

            {catalogue.length > 0 && (
              <details className="stg-fold">
                <summary className="stg-fold-head">
                  Available harnesses
                  <span className="stg-tag">{catalogue.length}</span>
                </summary>
                <p className="stg-hint">
                  Declared but off. Where a harness does not document its ACP launch command,
                  opman ships no command rather than a guessed one — open the row's docs link,
                  then fill the command in and enable it.
                </p>
                <AgentList agents={catalogue} {...listProps} />
              </details>
            )}
          </>
        )}
      </section>
    </div>
  );
}
