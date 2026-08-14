import React from "react";
import {
  Activity,
  Bot,
  CheckCircle2,
  Cpu,
  Layers,
  Octagon,
  ShieldCheck,
  TimerOff,
} from "lucide-react";
import { str } from "../tcUtils";
import { MetaChip, OpenSessionLink } from "./atoms";
import { agentRows, runnerOptions, shortId } from "./model";

/**
 * The shape-specific half of each card.
 *
 * One component per manager tool, each reading only the fields its own reply
 * carries. They are peers rather than one branching renderer because the
 * replies have nothing in common past the agent id — a single body would be a
 * pile of `&&`s guarding fields that never coexist.
 */

// ── agent_start ─────────────────────────────────────────

export function StartBody({
  sessionId,
  title,
  running,
}: {
  sessionId: string;
  title: string;
  running: boolean;
}) {
  return (
    <div className="am-start-banner">
      <Bot size={15} />
      <span className="am-start-text">
        {sessionId
          ? `Agent ${shortId(sessionId)} is ready`
          : running
            ? "Creating an agent session…"
            : "Agent session"}
      </span>
      <OpenSessionLink sessionId={sessionId} label={title} variant="button" />
    </div>
  );
}

// ── agent_list ──────────────────────────────────────────

export function ListBody({ output }: { output: Record<string, unknown> }) {
  const rows = agentRows(output);
  if (rows.length === 0) {
    return (
      <div className="am-card-muted">
        <CheckCircle2 size={11} /> No agent sessions in this project
      </div>
    );
  }
  return (
    <div className="am-list">
      {rows.map((row) => (
        <div className={`am-list-row${row.busy ? " is-busy" : ""}`} key={row.id}>
          <span className={`am-dot am-dot-${row.busy ? "busy" : "idle"}`} />
          <span className="am-list-title" title={row.title}>
            {row.title}
          </span>
          <span className="am-list-id" title={row.id}>
            {shortId(row.id)}
          </span>
          {row.runner && <span className="am-list-runner">{row.runner}</span>}
          {row.queued > 0 && <span className="am-list-queued">{row.queued} queued</span>}
          <OpenSessionLink sessionId={row.id} label={row.title} />
        </div>
      ))}
    </div>
  );
}

// ── agent_wait ──────────────────────────────────────────

export function WaitBody({
  output,
  sessionId,
  timeout,
}: {
  output: Record<string, unknown>;
  sessionId: string;
  timeout: string;
}) {
  const timedOut = output.timed_out === true;
  const reply = str(output.reply);
  return (
    <>
      <div className={`am-verdict am-verdict-${timedOut ? "timeout" : "done"}`}>
        {timedOut ? <TimerOff size={14} /> : <CheckCircle2 size={14} />}
        <span>{timedOut ? "Timed out — the turn is still running" : "Turn finished"}</span>
        {timeout && <span className="am-verdict-note">limit {timeout}s</span>}
        <OpenSessionLink sessionId={sessionId} label={shortId(sessionId) || "agent"} />
      </div>
      {reply && (
        <div className="am-reply">
          <div className="am-section-label">
            <Activity size={11} /> Reply
          </div>
          <div className="am-reply-text">{reply}</div>
        </div>
      )}
    </>
  );
}

// ── agent_abort ─────────────────────────────────────────

export function AbortBody({
  output,
  sessionId,
}: {
  output: Record<string, unknown>;
  sessionId: string;
}) {
  const aborted = output.aborted === true;
  return (
    <div className={`am-verdict am-verdict-${aborted ? "aborted" : "done"}`}>
      <Octagon size={14} />
      <span>{aborted ? "Turn cancelled — the session is intact" : "Nothing to cancel"}</span>
      <OpenSessionLink sessionId={sessionId} label={shortId(sessionId) || "agent"} />
    </div>
  );
}

// ── agent_runner_options ────────────────────────────────

/** The catalogue, capped: a runner can list hundreds and the card is a summary. */
const MODEL_LIMIT = 12;

export function OptionsBody({ output }: { output: Record<string, unknown> }) {
  const options = runnerOptions(output);
  const shown = options.models.slice(0, MODEL_LIMIT);
  const hidden = options.models.length - shown.length + options.omitted;

  return (
    <>
      <div className="am-meta-row">
        <MetaChip label="models" value={options.total ? String(options.total) : ""} icon={<Cpu size={10} />} />
        {options.connected.length > 0 && (
          <MetaChip label="connected" value={options.connected.join(", ")} />
        )}
        {options.efforts.length > 0 && <MetaChip label="efforts" value={options.efforts.join(" · ")} />}
      </div>

      {options.permissionModes.length > 0 && (
        <div className="am-tag-row">
          <div className="am-section-label">
            <ShieldCheck size={11} /> Permission modes
          </div>
          <div className="am-tags">
            {options.permissionModes.map((mode) => (
              <span className="am-tag" key={mode}>
                {mode}
              </span>
            ))}
          </div>
        </div>
      )}

      {shown.length > 0 && (
        <div className="am-models">
          {shown.map((model) => (
            <div className="am-model-row" key={`${model.provider}/${model.id}`}>
              <span className="am-model-name" title={model.name}>
                {model.name}
              </span>
              <span className="am-model-id" title={`${model.provider}/${model.id}`}>
                {model.id}
              </span>
              {model.efforts.length > 0 && (
                <span className="am-model-efforts">{model.efforts.join(" · ")}</span>
              )}
            </div>
          ))}
          {hidden > 0 && (
            <div className="am-card-muted">
              <Layers size={11} /> {hidden} more — narrow with `filter`
            </div>
          )}
        </div>
      )}

      {options.agents.length > 0 && (
        <div className="am-tag-row">
          <div className="am-section-label">
            <Bot size={11} /> Agents
          </div>
          <div className="am-tags">
            {options.agents.map((agent) => (
              <span className="am-tag" key={agent}>
                {agent}
              </span>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
