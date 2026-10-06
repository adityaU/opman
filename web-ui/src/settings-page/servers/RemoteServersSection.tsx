import { useState } from "react";
import { ExternalLink, Pencil, Plus, RefreshCw } from "lucide-react";
import { SERVER_ID } from "../../api/base";
import { useServers } from "../../hooks/useServers";
import { ServerDialog, type ServerDialogTarget } from "../../servers/ServerDialog";
import { STATUS_LABEL, ServerStatusDot, serverHost, switchToServer } from "../../servers/status";

/**
 * Servers: other machines running opman that this one relays.
 *
 * The same list as the sidebar's workspace menu, with room to say why a server is
 * unreachable. Adding and editing share the sidebar's dialog, so there is one form.
 */

export function RemoteServersSection() {
  const { servers, loading, error, refresh } = useServers();
  const [dialog, setDialog] = useState<ServerDialogTarget | null>(null);

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <div className="stg-card-head">
          <div>
            <h3 className="stg-card-title">opman servers</h3>
            <p className="stg-card-note">
              This server signs in to each one and relays it, so its projects open here next
              to this machine's. Credentials stay on this server.
            </p>
          </div>
          <div className="stg-card-actions">
            <button type="button" className="stg-btn" onClick={() => void refresh()} disabled={loading}>
              <RefreshCw size={13} aria-hidden="true" />
              Recheck
            </button>
            <button type="button" className="stg-btn is-primary" onClick={() => setDialog({ kind: "new" })}>
              <Plus size={13} aria-hidden="true" />
              Add server
            </button>
          </div>
        </div>

        {error && (
          <p className="stg-error" role="alert">
            {error}
          </p>
        )}

        {servers.length === 0 && loading ? (
          <p className="stg-hint">Loading…</p>
        ) : (
          <ul className="stg-rows">
            {servers.map((server) => {
              const current = server.id === SERVER_ID;
              return (
                <li key={server.id} className="stg-row">
                  <div className="stg-row-main">
                    <span className="stg-row-head">
                      <ServerStatusDot status={server.status} />
                      <span className="stg-row-name">{server.name}</span>
                      {current && <span className="stg-tag is-accent">this page</span>}
                      <span className="stg-tag">{STATUS_LABEL[server.status]}</span>
                    </span>
                    <span className="stg-row-origin">{server.url ?? serverHost(server)}</span>
                  </div>
                  <div className="stg-row-actions">
                    {!current && (
                      <button
                        type="button"
                        className="stg-icon-btn"
                        onClick={() => switchToServer(server.id)}
                        aria-label={`Open ${server.name}`}
                        title={`Open ${server.name}`}
                      >
                        <ExternalLink size={14} />
                      </button>
                    )}
                    {server.url && (
                      <button
                        type="button"
                        className="stg-icon-btn"
                        onClick={() => setDialog({ kind: "edit", server })}
                        aria-label={`Edit ${server.name}`}
                        title={`Edit ${server.name}`}
                      >
                        <Pencil size={14} />
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>
      {dialog && <ServerDialog target={dialog} onClose={() => setDialog(null)} />}
    </div>
  );
}
