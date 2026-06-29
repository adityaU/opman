import React, { useState, useRef, useCallback } from "react";
import { X, Plus, Trash2, ChevronUp, ChevronDown } from "lucide-react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { saveBoardConfig, type Board, type Lane, type Transitions } from "../api/kanban";
import type { AgentInfo } from "../api/session";

interface Props {
  board: Board;
  agents: AgentInfo[];
  onClose: () => void;
  onSaved: (board: Board) => void;
  onError: (msg: string) => void;
}

function genLaneId(): string {
  return `lane_${Math.random().toString(36).slice(2, 9)}`;
}

const DEFAULT_COLOR = "#7aa2f7";

export const LaneConfigModal: React.FC<Props> = function LaneConfigModal(p) {
  const modalRef = useRef<HTMLDivElement>(null);
  useEscape(p.onClose);
  useFocusTrap(modalRef);

  const [lanes, setLanes] = useState<Lane[]>(() => p.board.lanes.map((l) => ({ ...l })));
  const [transitions, setTransitions] = useState<Transitions>(() => ({ ...p.board.transitions }));
  const [saving, setSaving] = useState(false);

  const updateLane = useCallback((id: string, patch: Partial<Lane>) => {
    setLanes((prev) => prev.map((l) => (l.id === id ? { ...l, ...patch } : l)));
  }, []);

  const addLane = useCallback(() => {
    const id = genLaneId();
    setLanes((prev) => [
      ...prev,
      { id, name: "New lane", color: DEFAULT_COLOR, wip: null, terminal: false, agent: null, model: null, prompt: null },
    ]);
  }, []);

  const deleteLane = useCallback((id: string) => {
    setLanes((prev) => prev.filter((l) => l.id !== id));
    setTransitions((prev) => {
      const next: Transitions = {};
      for (const [from, tos] of Object.entries(prev)) {
        if (from === id) continue;
        next[from] = tos.filter((t) => t !== id);
      }
      return next;
    });
  }, []);

  const moveLane = useCallback((index: number, dir: -1 | 1) => {
    setLanes((prev) => {
      const target = index + dir;
      if (target < 0 || target >= prev.length) return prev;
      const next = [...prev];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }, []);

  /** Only one lane may be terminal. */
  const setTerminal = useCallback((id: string) => {
    setLanes((prev) => prev.map((l) => ({ ...l, terminal: l.id === id })));
  }, []);

  const toggleTransition = useCallback((from: string, to: string) => {
    setTransitions((prev) => {
      const existing = prev[from] ?? [];
      const has = existing.includes(to);
      return {
        ...prev,
        [from]: has ? existing.filter((t) => t !== to) : [...existing, to],
      };
    });
  }, []);

  const handleSave = useCallback(async () => {
    if (lanes.length === 0) {
      p.onError("At least one lane is required.");
      return;
    }
    setSaving(true);
    try {
      const res = await saveBoardConfig(p.board.id, lanes, transitions);
      p.onSaved(res.board);
      p.onClose();
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to save board config");
      setSaving(false);
    }
  }, [lanes, transitions, p]);

  return (
    <div className="kanban-modal-overlay" onClick={p.onClose}>
      <div
        ref={modalRef}
        className="kanban-modal kanban-modal-lg liquid-glass"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="kanban-modal-header">
          <h3>Configure lanes</h3>
          <button className="kanban-modal-close" onClick={p.onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>

        <div className="kanban-modal-body">
          {/* ── Lane editor ── */}
          <div className="kanban-config-section">
            <div className="kanban-config-section-head">
              <h4>Lanes</h4>
              <button className="kanban-btn kanban-btn-sm" onClick={addLane}>
                <Plus size={12} /> Add lane
              </button>
            </div>

            <div className="kanban-lane-rows">
              {lanes.map((lane, i) => (
                <div key={lane.id} className="kanban-lane-item">
                <div className="kanban-lane-row">
                  <div className="kanban-lane-row-reorder">
                    <button onClick={() => moveLane(i, -1)} disabled={i === 0} aria-label="Move up">
                      <ChevronUp size={12} />
                    </button>
                    <button
                      onClick={() => moveLane(i, 1)}
                      disabled={i === lanes.length - 1}
                      aria-label="Move down"
                    >
                      <ChevronDown size={12} />
                    </button>
                  </div>
                  <input
                    type="color"
                    className="kanban-color-input"
                    value={lane.color}
                    onChange={(e) => updateLane(lane.id, { color: e.target.value })}
                    aria-label="Lane color"
                  />
                  <input
                    className="kanban-input kanban-lane-row-name"
                    value={lane.name}
                    onChange={(e) => updateLane(lane.id, { name: e.target.value })}
                    placeholder="Lane name"
                  />
                  <input
                    className="kanban-input kanban-lane-row-wip"
                    type="number"
                    min={0}
                    value={lane.wip ?? ""}
                    onChange={(e) =>
                      updateLane(lane.id, {
                        wip: e.target.value === "" ? null : Number(e.target.value),
                      })
                    }
                    placeholder="WIP"
                    title="WIP limit (blank = none)"
                  />
                  <select
                    className="kanban-input kanban-lane-row-agent"
                    value={lane.agent ?? ""}
                    onChange={(e) =>
                      updateLane(lane.id, { agent: e.target.value === "" ? null : e.target.value })
                    }
                    title="Default agent for this lane"
                  >
                    <option value="">(no agent)</option>
                    {lane.agent && !p.agents.some((a) => a.id === lane.agent) && (
                      <option value={lane.agent}>{lane.agent}</option>
                    )}
                    {p.agents.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.label || a.id}
                      </option>
                    ))}
                  </select>
                  <input
                    className="kanban-input kanban-lane-row-model"
                    value={lane.model ?? ""}
                    onChange={(e) =>
                      updateLane(lane.id, { model: e.target.value === "" ? null : e.target.value })
                    }
                    placeholder="Model (optional)"
                    title="Default model for this lane"
                  />
                  <label className="kanban-lane-row-terminal" title="Terminal review lane">
                    <input
                      type="radio"
                      name="kanban-terminal-lane"
                      checked={lane.terminal}
                      onChange={() => setTerminal(lane.id)}
                    />
                    <span>Review</span>
                  </label>
                  <button
                    className="kanban-icon-btn kanban-icon-btn-danger"
                    onClick={() => deleteLane(lane.id)}
                    aria-label="Delete lane"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
                  <textarea
                    className="kanban-input kanban-lane-prompt"
                    value={lane.prompt ?? ""}
                    onChange={(e) =>
                      updateLane(lane.id, { prompt: e.target.value === "" ? null : e.target.value })
                    }
                    placeholder={`Pipeline prompt for "${lane.name}" — used when launching in Pipeline mode. The previous stage's output is appended automatically.`}
                    rows={2}
                  />
                </div>
              ))}
            </div>
          </div>

          {/* ── Transition matrix (rows = from, cols = to) ── */}
          <div className="kanban-config-section">
            <div className="kanban-config-section-head">
              <h4>Allowed transitions (from → to)</h4>
            </div>
            <div className="kanban-matrix-scroll">
              <table className="kanban-matrix">
                <thead>
                  <tr>
                    <th className="kanban-matrix-corner">from \ to</th>
                    {lanes.map((l) => (
                      <th key={l.id} title={l.name}>
                        <span className="kanban-matrix-dot" style={{ background: l.color }} />
                        {l.name}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {lanes.map((from) => (
                    <tr key={from.id}>
                      <th title={from.name}>
                        <span className="kanban-matrix-dot" style={{ background: from.color }} />
                        {from.name}
                      </th>
                      {lanes.map((to) => (
                        <td key={to.id}>
                          {from.id === to.id ? (
                            <span className="kanban-matrix-self">—</span>
                          ) : (
                            <input
                              type="checkbox"
                              checked={(transitions[from.id] ?? []).includes(to.id)}
                              onChange={() => toggleTransition(from.id, to.id)}
                              aria-label={`Allow ${from.name} to ${to.name}`}
                            />
                          )}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <div className="kanban-modal-footer">
          <div className="kanban-modal-footer-right">
            <button className="kanban-btn" onClick={p.onClose} disabled={saving}>
              Cancel
            </button>
            <button className="kanban-btn kanban-btn-primary" onClick={handleSave} disabled={saving}>
              {saving ? "Saving…" : "Save config"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
