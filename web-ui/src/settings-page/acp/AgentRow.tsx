import React from "react";
import { BookOpen, CircleCheck, CircleX, LogIn, Loader, Pencil, RefreshCw, RotateCcw, Trash2 } from "lucide-react";
import type { AcpAgent, NativeToolsSetting } from "../../api/acp";
import { NativeToolsPanel } from "./NativeToolsPanel";
import { authStatusOf, originOf, statusOf } from "./status";
import type { AuthEntry } from "./useAcpAuth";

/**
 * One declared agent.
 *
 * An ACP agent is a runner, so the row leads with the thing that decides whether it can be
 * picked — its status — and then says what would be spawned. The rest is the handful of
 * settings that change behaviour rather than merely describing it.
 */

export interface AgentRowProps {
  readonly agent: AcpAgent;
  readonly busy: boolean;
  readonly editing: boolean;
  readonly onToggle: () => void;
  readonly onEdit: () => void;
  readonly onNativeTools: (setting: NativeToolsSetting) => void;
  /** Login state, once asked. Absent for an agent that is off. */
  readonly auth?: AuthEntry;
  /** Open a terminal pane running this agent's login command. */
  readonly onLogin: (command: string) => void;
  readonly onRecheck: () => void;
  /** Remove the user's entry: a delete for a declared agent, a reset for a built-in. */
  readonly onRemove: () => void;
}

/** The behaviour facts worth a line each. Silence means "the default", which is the norm. */
function facts(agent: AcpAgent): string[] {
  const caps = [
    agent.clientCaps.readTextFile && "read",
    agent.clientCaps.writeTextFile && "write",
    agent.clientCaps.terminal && "terminal",
  ].filter(Boolean);
  return [
    `runner: ${agent.runner}`,
    agent.defaultMode && `mode: ${agent.defaultMode}`,
    agent.defaultModel && `model: ${agent.defaultModel}`,
    agent.modesAreAgents && "modes are agents",
    agent.subagentTranscripts && "nests subagent sessions",
    !agent.injectMcp && "no opman MCP servers",
    caps.length > 0 && `opman handles: ${caps.join(", ")}`,
    agent.envNames.length > 0 && `env: ${agent.envNames.join(", ")}`,
    agent.envRemove.length > 0 && `strips: ${agent.envRemove.join(", ")}`,
  ].filter((entry): entry is string => Boolean(entry));
}

export function AgentRow({
  agent, busy, editing, onToggle, onEdit, onNativeTools, onRemove, auth, onLogin, onRecheck,
}: AgentRowProps) {
  const status = statusOf(agent);
  const login = auth ? authStatusOf(auth) : undefined;

  return (
    <li className={busy ? "stg-row is-busy" : "stg-row"}>
      <div className="stg-row-main">
        <div className="stg-row-head">
          <span className="stg-row-name">{agent.displayName}</span>
          <span className="stg-tag">{agent.id}</span>
          {agent.builtin && <span className="stg-tag">built-in</span>}
          {agent.builtin && agent.customized && <span className="stg-tag is-accent">edited</span>}
          {agent.isDefault && <span className="stg-tag is-accent">default runner</span>}
          <span className={`stg-pill is-${status.tone}`}>
            {status.icon}
            {status.label}
          </span>
          {login && (
            <span className={`stg-pill is-${login.tone}`} role="status">
              {login.checking ? <Loader size={11} aria-hidden="true" /> : login.connected ? <CircleCheck size={11} aria-hidden="true" /> : <CircleX size={11} aria-hidden="true" />}
              {login.label}
            </span>
          )}
        </div>

        {login && !login.checking && !login.connected && login.detail && (
          <p className="stg-row-caveat">{login.detail}</p>
        )}

        <p className="stg-row-origin" title={originOf(agent)}>
          {originOf(agent)}
        </p>

        <div className="stg-row-facts">
          {facts(agent).map((fact) => (
            <span key={fact}>{fact}</span>
          ))}
        </div>

        {/* A catalogued agent whose upstream docs never state the launch command ships with
            none, so the docs link is the only thing that can move it forward. Say so where
            the missing command is, rather than leaving the row looking broken. */}
        {agent.builtin && !agent.command && agent.docs && (
          <p className="stg-row-caveat">
            <BookOpen size={11} aria-hidden="true" />
            opman could not find a documented ACP command for this one, and will not guess at
            a binary to spawn under its name.{" "}
            <a href={agent.docs} target="_blank" rel="noreferrer noopener">
              Check its documentation
            </a>
            , then edit the row to add the command.
          </p>
        )}

        {agent.slotTaken && (
          <p className="stg-row-caveat">
            The <code>{agent.runner}</code> runner is served by another engine, so this agent
            cannot claim it. Give it a slot of its own and it will start.
          </p>
        )}

        {agent.lastError && (
          <div className="stg-row-failure" role="alert">
            <span className="stg-row-failure-title">Failed to start</span>
            <pre className="stg-row-failure-text">{agent.lastError}</pre>
          </div>
        )}

        {agent.enabled && (
          <NativeToolsPanel
            agentName={agent.displayName}
            info={agent.nativeTools}
            disabled={busy}
            onChange={onNativeTools}
          />
        )}

        {agent.isDefault && (
          <p className="stg-row-caveat">
            <RefreshCw size={11} aria-hidden="true" />
            opman itself started on this engine, so a change here is saved but applied on the
            next start. Every other agent takes effect immediately.
          </p>
        )}
      </div>

      <div className="stg-row-actions">
        {login?.command && (
          <button
            type="button"
            className="stg-btn is-primary"
            onClick={() => onLogin(login.command ?? "")}
            title="Opens a terminal pane that logs this agent in"
          >
            <LogIn size={13} aria-hidden="true" />
            Log in
          </button>
        )}
        {login && !login.checking && (
          <button
            type="button"
            className="stg-icon-btn"
            onClick={onRecheck}
            aria-label={`Check ${agent.displayName}'s login again`}
            title="Check the login again"
          >
            <RefreshCw size={14} />
          </button>
        )}

        <button
          type="button"
          className="stg-switch"
          role="switch"
          aria-checked={agent.enabled}
          aria-label={`${agent.enabled ? "Disable" : "Enable"} ${agent.displayName}`}
          onClick={onToggle}
        >
          <span className="stg-switch-track" aria-hidden="true">
            <span className="stg-switch-knob" />
          </span>
          <span className="stg-switch-text">{agent.enabled ? "On" : "Off"}</span>
        </button>

        <button
          type="button"
          className={editing ? "stg-icon-btn is-active" : "stg-icon-btn"}
          onClick={onEdit}
          aria-label={`Edit ${agent.displayName}`}
          aria-expanded={editing}
        >
          <Pencil size={14} />
        </button>

        {/* A built-in cannot be deleted — dropping its entry restores opman's own
            definition — so it gets a restore rather than a red Remove that would lie
            about what happens. An untouched built-in has no entry to drop at all. */}
        {agent.builtin ? (
          agent.customized && (
            <button
              type="button"
              className="stg-icon-btn"
              onClick={onRemove}
              aria-label={`Restore the built-in ${agent.displayName}`}
              title="Discard your changes and restore opman's definition"
            >
              <RotateCcw size={14} />
            </button>
          )
        ) : (
          <button
            type="button"
            className="stg-icon-btn is-danger"
            onClick={onRemove}
            aria-label={`Remove ${agent.displayName}`}
          >
            <Trash2 size={14} />
          </button>
        )}
      </div>
    </li>
  );
}
