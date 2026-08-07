import React from "react";
import { Lock, Pencil, RefreshCw, Trash2, Wrench } from "lucide-react";
import type { McpServer } from "../../api/mcp";
import { LoginFlow } from "./LoginFlow";
import { originOf, statusOf } from "./status";
import { ToolCatalog } from "./ToolCatalog";

/**
 * One declared server.
 *
 * The row's job is to make three things answerable without opening anything: is it on, can
 * it be reached, and does opman hold its credential. The fourth question — what it
 * actually gives an agent — costs a launched process to answer, so it is one click away
 * rather than always on.
 */

export interface ServerRowProps {
  readonly server: McpServer;
  readonly busy: boolean;
  readonly editing: boolean;
  /** Whether the tool catalog is open for this row. */
  readonly showingTools: boolean;
  readonly onToggle: () => void;
  readonly onEdit: () => void;
  readonly onShowTools: () => void;
  readonly onRemove: () => void;
  readonly onError: (message: string) => void;
}

export function ServerRow(props: ServerRowProps) {
  const { server, busy, editing, showingTools } = props;
  const status = statusOf(server);

  return (
    <li className={busy ? "stg-row is-busy" : "stg-row"}>
      <div className="stg-row-main">
        <div className="stg-row-head">
          <span className="stg-row-name">{server.name}</span>
          <span className="stg-tag">{server.transport}</span>
          {server.builtin && <span className="stg-tag">built-in</span>}
          {server.proxied && (
            <span className="stg-tag is-accent" title="opman mints the credential; the runner never sees it">
              <Lock size={11} aria-hidden="true" />
              proxied
            </span>
          )}
          <span className={`stg-pill is-${status.tone}`}>
            {status.icon}
            {status.label}
          </span>
        </div>

        <p className="stg-row-origin" title={originOf(server)}>
          {originOf(server)}
        </p>

        <div className="stg-row-facts">
          {server.runners.length > 0 ? (
            <span>Only {server.runners.join(", ")}</span>
          ) : (
            <span>All runners</span>
          )}
          {server.timeoutSecs != null && <span>{server.timeoutSecs}s tool timeout</span>}
          {server.envNames.length > 0 && <span>env: {server.envNames.join(", ")}</span>}
          {server.headerNames.length > 0 && (
            <span>headers: {server.headerNames.join(", ")}</span>
          )}
        </div>

        {server.enabled && server.needsOpencodeRestart && (
          <p className="stg-row-caveat">
            <RefreshCw size={11} aria-hidden="true" />
            OpenCode is handed its config once at spawn, so this lands there on its next
            start. The other runners pick it up on the next turn.
          </p>
        )}

        {server.auth === "oauth" && (
          <LoginFlow
            name={server.name}
            authenticated={server.authenticated}
            onError={props.onError}
          />
        )}

        <ToolCatalog name={server.name} open={showingTools} />
      </div>

      <div className="stg-row-actions">
        {/* Reads what the server offers, which means starting it — so it is a deliberate
            click, and its label says what it will do rather than just naming a noun. */}
        <button
          type="button"
          className={showingTools ? "stg-icon-btn is-active" : "stg-icon-btn"}
          onClick={props.onShowTools}
          aria-label={`${showingTools ? "Hide" : "Show"} the tools ${server.name} offers`}
          aria-expanded={showingTools}
          title="Tools this server offers"
        >
          <Wrench size={14} />
        </button>

        <button
          type="button"
          className="stg-switch"
          role="switch"
          aria-checked={server.enabled}
          aria-label={`${server.enabled ? "Disable" : "Enable"} ${server.name}`}
          onClick={props.onToggle}
        >
          <span className="stg-switch-track" aria-hidden="true">
            <span className="stg-switch-knob" />
          </span>
          <span className="stg-switch-text">{server.enabled ? "On" : "Off"}</span>
        </button>

        <button
          type="button"
          className={editing ? "stg-icon-btn is-active" : "stg-icon-btn"}
          onClick={props.onEdit}
          aria-label={`Edit ${server.name}`}
          aria-expanded={editing}
        >
          <Pencil size={14} />
        </button>

        {/* A built-in has no entry to delete — removing its config only restores opman's
            own definition, so offering a red Remove would misdescribe what happens. */}
        {!server.builtin && (
          <button
            type="button"
            className="stg-icon-btn is-danger"
            onClick={props.onRemove}
            aria-label={`Remove ${server.name}`}
          >
            <Trash2 size={14} />
          </button>
        )}
      </div>
    </li>
  );
}
