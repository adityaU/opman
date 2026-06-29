import React, { useState, useRef, useMemo, useEffect, useCallback } from "react";
import { X, Play, User, GitBranch } from "lucide-react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { useProviders } from "../hooks/useProviders";
import { fetchAgents, type AgentInfo } from "../api/session";
import { launchTask, type Task, type Lane, type Board, type LaunchMode } from "../api/kanban";

interface Props {
  task: Task;
  /** The full board — supplies the lane order for pipeline-mode stages. */
  board: Board;
  /** The task's current lane — supplies the default agent + model. */
  lane: Lane | undefined;
  onClose: () => void;
  /** Called with the new session id after a successful launch. */
  onLaunched: (sessionId: string) => void;
  onError: (msg: string) => void;
}

interface FlatModel {
  modelId: string;
  label: string;
}

export const LaunchModal: React.FC<Props> = function LaunchModal(p) {
  const modalRef = useRef<HTMLDivElement>(null);
  useEscape(p.onClose);
  useFocusTrap(modalRef);

  const providers = useProviders();
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  // Pre-select agent/model resolved from the task's current lane; both editable.
  const [agent, setAgent] = useState<string>(p.lane?.agent ?? "");
  const [model, setModel] = useState<string>(p.lane?.model ?? "");
  const [mode, setMode] = useState<LaunchMode>("single");
  const [launching, setLaunching] = useState(false);
  const [launchedSession, setLaunchedSession] = useState<string | null>(null);

  // Pipeline stages: lanes from the current lane forward, excluding the terminal
  // review lane, that have an agent or a prompt configured (mirrors the backend).
  const stageLanes = useMemo(() => {
    const start = Math.max(
      0,
      p.board.lanes.findIndex((l) => l.id === p.task.lane_id),
    );
    return p.board.lanes
      .slice(start)
      .filter((l) => !l.terminal && (l.agent || (l.prompt && l.prompt.trim())));
  }, [p.board.lanes, p.task.lane_id]);

  useEffect(() => {
    let alive = true;
    fetchAgents().then((a) => {
      if (alive) setAgents(a.filter((x) => !x.hidden));
    });
    return () => {
      alive = false;
    };
  }, []);

  const models = useMemo<FlatModel[]>(() => {
    const result: FlatModel[] = [];
    for (const prov of providers.all) {
      if (!providers.connected.has(prov.id)) continue;
      if (!prov.models) continue;
      for (const [modelId, info] of Object.entries(prov.models)) {
        result.push({
          modelId,
          label: `${info.name || modelId} (${prov.name || prov.id})`,
        });
      }
    }
    return result;
  }, [providers.all, providers.connected]);

  const handleLaunch = useCallback(async () => {
    setLaunching(true);
    try {
      const body: { model?: string; agent?: string; mode?: LaunchMode } = { mode };
      if (model) body.model = model;
      if (agent) body.agent = agent;
      const res = await launchTask(p.task.id, body);
      setLaunchedSession(res.session_id);
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to launch task");
      setLaunching(false);
    }
  }, [model, agent, mode, p]);

  return (
    <div className="kanban-modal-overlay" onClick={p.onClose}>
      <div
        ref={modalRef}
        className="kanban-modal kanban-modal-sm liquid-glass"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="kanban-modal-header">
          <h3>Launch task</h3>
          <button className="kanban-modal-close" onClick={p.onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>

        <div className="kanban-modal-body">
          <div className="kanban-launch-summary">
            <span className="kanban-launch-summary-title">{p.task.title}</span>
            {p.lane && <span className="kanban-launch-summary-lane">in {p.lane.name}</span>}
          </div>

          {!launchedSession && (
            <div className="kanban-mode-toggle" role="radiogroup" aria-label="Launch mode">
              <button
                type="button"
                role="radio"
                aria-checked={mode === "single"}
                className={`kanban-mode-option${mode === "single" ? " is-active" : ""}`}
                onClick={() => setMode("single")}
              >
                <User size={14} />
                <span className="kanban-mode-title">Single session</span>
                <span className="kanban-mode-desc">One agent walks the whole board.</span>
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={mode === "pipeline"}
                className={`kanban-mode-option${mode === "pipeline" ? " is-active" : ""}`}
                onClick={() => setMode("pipeline")}
              >
                <GitBranch size={14} />
                <span className="kanban-mode-title">Pipeline</span>
                <span className="kanban-mode-desc">Each lane runs in its own session, chained by output.</span>
              </button>
            </div>
          )}

          {!launchedSession && mode === "pipeline" && (
            <div className="kanban-pipeline-preview">
              <span className="kanban-field-label">
                Stages ({stageLanes.length})
              </span>
              {stageLanes.length === 0 ? (
                <p className="kanban-pipeline-empty">
                  No stages — give lanes an agent or a prompt in “Configure lanes”.
                </p>
              ) : (
                <ol className="kanban-pipeline-stages">
                  {stageLanes.map((l, i) => (
                    <li key={l.id} className="kanban-pipeline-stage">
                      <span className="kanban-pipeline-stage-dot" style={{ background: l.color }} />
                      <span className="kanban-pipeline-stage-name">{l.name}</span>
                      {l.agent && <span className="kanban-pipeline-stage-agent">{l.agent}</span>}
                      {!l.prompt?.trim() && (
                        <span className="kanban-pipeline-stage-warn" title="No stage prompt set; a generic one is used">
                          no prompt
                        </span>
                      )}
                      {i < stageLanes.length - 1 && <span className="kanban-pipeline-arrow">→</span>}
                    </li>
                  ))}
                </ol>
              )}
            </div>
          )}

          {launchedSession ? (
            <div className="kanban-launch-done">
              <p>Session started.</p>
              <button
                className="kanban-btn kanban-btn-primary"
                onClick={() => p.onLaunched(launchedSession)}
              >
                Open session
              </button>
            </div>
          ) : (
            <>
              <label className="kanban-field">
                <span className="kanban-field-label">
                  Agent {p.lane?.agent ? `(default for ${p.lane.name})` : ""}
                </span>
                <select
                  className="kanban-input"
                  value={agent}
                  onChange={(e) => setAgent(e.target.value)}
                >
                  <option value="">(engine default)</option>
                  {/* Ensure the lane's configured agent is selectable even if not in /api/agents. */}
                  {p.lane?.agent && !agents.some((a) => a.id === p.lane?.agent) && (
                    <option value={p.lane.agent}>{p.lane.agent}</option>
                  )}
                  {agents.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.label || a.id}
                    </option>
                  ))}
                </select>
              </label>

              <label className="kanban-field">
                <span className="kanban-field-label">Model</span>
                <select
                  className="kanban-input"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                >
                  <option value="">(default model)</option>
                  {p.lane?.model && !models.some((m) => m.modelId === p.lane?.model) && (
                    <option value={p.lane.model}>{p.lane.model}</option>
                  )}
                  {models.map((m) => (
                    <option key={m.modelId} value={m.modelId}>
                      {m.label}
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}
        </div>

        {!launchedSession && (
          <div className="kanban-modal-footer">
            <div className="kanban-modal-footer-right">
              <button className="kanban-btn" onClick={p.onClose} disabled={launching}>
                Cancel
              </button>
              <button
                className="kanban-btn kanban-btn-primary"
                onClick={handleLaunch}
                disabled={launching || (mode === "pipeline" && stageLanes.length === 0)}
              >
                <Play size={13} />{" "}
                {launching ? "Launching…" : mode === "pipeline" ? "Launch pipeline" : "Launch"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
