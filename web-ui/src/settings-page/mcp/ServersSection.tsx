import React, { useCallback, useState } from "react";
import { Plus } from "lucide-react";
import type { McpServer } from "../../api/mcp";
import { ServerForm } from "./ServerForm";
import { ServerRow } from "./ServerRow";
import { useMcpServers } from "./useMcpServers";

/**
 * MCP servers: the tools every runner can reach.
 *
 * One list, because that is the point of the registry — a server declared once is fanned
 * out to all four runners in their own wire shapes, so there is nothing per-runner to show
 * beyond which of them a server is withheld from.
 */

/** The runner slots to offer when the page was opened without the live list. */
const FALLBACK_RUNNERS: readonly string[] = ["opencode", "claude-code", "claude", "codex"];

export interface ServersSectionProps {
  readonly onError: (message: string) => void;
  /** Live runner list, including any ACP agent. */
  readonly runners?: readonly string[];
}

type Editing = { readonly kind: "none" } | { readonly kind: "new" } | { readonly kind: "server"; readonly name: string };

export function ServersSection({ onError, runners }: ServersSectionProps) {
  const state = useMcpServers(onError);
  const [editing, setEditing] = useState<Editing>({ kind: "none" });
  const [confirming, setConfirming] = useState<string>();
  // One at a time: each open catalog holds a launched server, and two rows of schema
  // tables is more than fits on screen anyway.
  const [showingTools, setShowingTools] = useState<string>();
  const slots = runners && runners.length > 0 ? runners : FALLBACK_RUNNERS;

  const close = useCallback(() => setEditing({ kind: "none" }), []);

  const remove = useCallback(
    async (name: string) => {
      setConfirming(undefined);
      await state.remove(name);
      setEditing((current) =>
        current.kind === "server" && current.name === name ? { kind: "none" } : current,
      );
    },
    [state],
  );

  const editingServer = (server: McpServer) =>
    editing.kind === "server" && editing.name === server.name;

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <div className="stg-card-head">
          <div>
            <h3 className="stg-card-title">Declared servers</h3>
            <p className="stg-card-note">
              A server declared here is offered to every runner in its own configuration
              format. Anything carrying a credential is fronted by opman's local proxy, so
              the secret never reaches a runner.
            </p>
          </div>
          <button
            type="button"
            className="stg-btn is-primary"
            onClick={() => setEditing(editing.kind === "new" ? { kind: "none" } : { kind: "new" })}
            aria-expanded={editing.kind === "new"}
          >
            <Plus size={13} aria-hidden="true" />
            Add server
          </button>
        </div>

        {editing.kind === "new" && (
          <ServerForm
            runners={slots}
            saving={state.busy !== undefined}
            onSubmit={state.save}
            onCancel={close}
          />
        )}

        {state.error && (
          <p className="stg-error" role="alert">
            {state.error}
          </p>
        )}

        {state.loading ? (
          <p className="stg-hint">Loading…</p>
        ) : state.servers.length === 0 ? (
          <div className="stg-empty">
            <p>No MCP servers yet.</p>
            <p className="stg-hint">
              Add one and every runner gets it on its next turn — no per-runner config file
              to keep in step.
            </p>
          </div>
        ) : (
          <ul className="stg-rows">
            {state.servers.map((server) => (
              <React.Fragment key={server.name}>
                <ServerRow
                  server={server}
                  busy={state.busy === server.name}
                  editing={editingServer(server)}
                  showingTools={showingTools === server.name}
                  onToggle={() => state.toggle(server)}
                  onEdit={() =>
                    setEditing(
                      editingServer(server) ? { kind: "none" } : { kind: "server", name: server.name },
                    )
                  }
                  onShowTools={() =>
                    setShowingTools((current) =>
                      current === server.name ? undefined : server.name,
                    )
                  }
                  onRemove={() => setConfirming(server.name)}
                  onError={onError}
                />
                {confirming === server.name && (
                  <li className="stg-confirm">
                    <span>
                      Remove <strong>{server.name}</strong> from <code>mcp.json</code>? Any
                      stored credential for it is left in place — sign out first to drop
                      that too.
                    </span>
                    <span className="stg-confirm-actions">
                      <button
                        type="button"
                        className="stg-btn is-danger"
                        onClick={() => remove(server.name)}
                      >
                        Remove
                      </button>
                      <button
                        type="button"
                        className="stg-btn"
                        onClick={() => setConfirming(undefined)}
                      >
                        Keep
                      </button>
                    </span>
                  </li>
                )}
                {editingServer(server) && (
                  <li className="stg-row-form">
                    <ServerForm
                      server={server}
                      runners={slots}
                      saving={state.busy === server.name}
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
