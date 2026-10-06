import { Check, Pencil, Server } from "lucide-react";
import { IS_HOME, SERVER_ID } from "../api/base";
import type { ServerInfo } from "../api/servers";
import { useServers } from "../hooks/useServers";
import type { ServerDialogTarget } from "./ServerDialog";
import { STATUS_LABEL, ServerStatusDot, serverHost, switchToServer } from "./status";

/**
 * The server half of the workspace menu.
 *
 * A server is the outermost thing a workspace lives on, so it sits above the projects in
 * the same menu rather than in a control of its own: with one server there is nothing to
 * choose and the whole section is a single "Add server…" row at the foot.
 */

interface Props {
  /** Open the add/edit dialog. The menu closes itself first. */
  readonly onOpenDialog: (target: ServerDialogTarget) => void;
}

/** The list, shown above the projects when there is more than one server. */
export function ServerMenuList({ onOpenDialog }: Props) {
  const { servers, multi } = useServers();
  if (!multi) return null;

  return (
    <div className="sb-server-group" role="group" aria-label="Servers">
      <div className="sb-server-eyebrow">Servers</div>
      {servers.map((server) => (
        <ServerMenuRow key={server.id} server={server} onOpenDialog={onOpenDialog} />
      ))}
    </div>
  );
}

function ServerMenuRow({ server, onOpenDialog }: Props & { readonly server: ServerInfo }) {
  const active = server.id === SERVER_ID;
  const label = `${server.name} — ${STATUS_LABEL[server.status]}`;
  return (
    <div className="sb-server-row">
      <button
        type="button"
        role="menuitemradio"
        aria-checked={active}
        aria-label={label}
        title={server.url ?? label}
        className={`sb-project-item sb-server-item${active ? " is-active" : ""}`}
        onClick={() => {
          if (!active) switchToServer(server.id);
        }}
      >
        <ServerStatusDot status={server.status} />
        <span className="sb-project-item-name">{server.name}</span>
        <span className="sb-server-host">{serverHost(server)}</span>
        {active && <Check size={12} className="sb-project-item-check" />}
      </button>
      {server.url && (
        <button
          type="button"
          role="menuitem"
          className="sb-server-edit"
          aria-label={`Edit ${server.name}`}
          title={`Edit ${server.name}`}
          onClick={() => onOpenDialog({ kind: "edit", server })}
        >
          <Pencil size={11} />
        </button>
      )}
    </div>
  );
}

/** The foot row that adds one. Always present, so the feature is findable with one server. */
export function AddServerItem({ onOpenDialog }: Props) {
  return (
    <button
      type="button"
      role="menuitem"
      className="sb-project-item sb-project-add sb-server-add"
      onClick={() => onOpenDialog({ kind: "new" })}
    >
      <Server size={12} />
      <span>Add server…</span>
    </button>
  );
}

/** The current server's name for the trigger, or nothing when there is only one. */
export function ServerBadge() {
  const { current, multi } = useServers();
  if (!multi || !current) return null;
  return (
    <span
      className={`sb-server-badge${IS_HOME ? " is-home" : ""}`}
      title={`${current.name} — ${STATUS_LABEL[current.status]}`}
    >
      <ServerStatusDot status={current.status} />
      {!IS_HOME && <span className="sb-server-badge-name">{current.name}</span>}
    </span>
  );
}
