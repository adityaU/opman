import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Target, X, Plus, Trash2, Play, Pause, RotateCcw, Ban, ChevronDown, ChevronUp } from "lucide-react";
import {
  createMission,
  deleteMission,
  fetchMissions,
  updateMission,
  missionAction,
} from "../api";
import type { Mission, MissionState, MissionAction } from "../api";
import {
  formatState,
  stateColor,
  formatVerdict,
  formatRelativeDate,
  canPerformAction,
} from "./helpers";
import { STATE_ORDER } from "./types";
import type { MissionsModalProps } from "./types";

export function MissionsModal({
  onClose,
  projects,
  activeProjectIndex,
  activeSessionId,
  activeMemoryItems = [],
}: MissionsModalProps) {
  const [missions, setMissions] = useState<Mission[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [goal, setGoal] = useState("");
  const [maxIterations, setMaxIterations] = useState(10);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const loadMissions = useCallback(async () => {
    try {
      const resp = await fetchMissions();
      setMissions(resp.missions);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadMissions();
  }, [loadMissions]);

  const grouped = useMemo(() => {
    return STATE_ORDER.map((current) => ({
      state: current,
      items: missions.filter((m) => m.state === current),
    }));
  }, [missions]);

  const handleCreate = useCallback(async () => {
    const trimmedGoal = goal.trim();
    if (!trimmedGoal) return;

    setSaving(true);
    try {
      await createMission({
        goal: trimmedGoal,
        session_id: activeSessionId,
        project_index: activeProjectIndex,
        max_iterations: maxIterations,
      });
      setGoal("");
      setMaxIterations(10);
      await loadMissions();
    } finally {
      setSaving(false);
    }
  }, [goal, maxIterations, activeProjectIndex, activeSessionId, loadMissions]);

  const handleAction = useCallback(async (mission: Mission, action: MissionAction) => {
    try {
      const updated = await missionAction(mission.id, action);
      setMissions((prev) => prev.map((m) => (m.id === updated.id ? updated : m)));
    } catch {
      // ignore
    }
  }, []);

  const handleDelete = useCallback(async (missionId: string) => {
    try {
      await deleteMission(missionId);
      setMissions((prev) => prev.filter((m) => m.id !== missionId));
    } catch {
      // ignore
    }
  }, []);

  const handleGoalBlur = useCallback(async (mission: Mission, value: string) => {
    const trimmed = value.trim();
    if (trimmed === mission.goal || !trimmed) return;
    try {
      const updated = await updateMission(mission.id, { goal: trimmed });
      setMissions((prev) => prev.map((m) => (m.id === updated.id ? updated : m)));
    } catch {
      // ignore
    }
  }, []);

  const projectName = projects[activeProjectIndex]?.name ?? `Project ${activeProjectIndex}`;

  return (
    <div className="missions-overlay" onClick={onClose}>
      <div ref={modalRef} className="missions-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="missions-header">
          <div className="missions-header-left">
            <Target size={16} />
            <h3>Missions</h3>
            <span className="missions-count">{missions.length}</span>
          </div>
          <button onClick={onClose} aria-label="Close missions">
            <X size={16} />
          </button>
        </div>

        <div className="missions-create">
          <textarea
            className="missions-textarea"
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            placeholder="Describe the goal for this mission..."
            rows={3}
          />
          <div className="missions-create-grid">
            <label className="missions-iterations-label">
              Max iterations:
              <input
                className="missions-input missions-iterations-input"
                type="number"
                min={0}
                max={100}
                value={maxIterations}
                onChange={(e) => setMaxIterations(Number(e.target.value) || 0)}
              />
              <span className="missions-iterations-hint">
                {maxIterations === 0 ? "(unlimited)" : ""}
              </span>
            </label>
          </div>
          <div className="missions-create-footer">
            <span className="missions-context">
              {projectName}
              {activeSessionId ? ` \u2022 session ${activeSessionId.slice(0, 8)}` : " \u2022 no session linked"}
            </span>
            <button
              className="missions-create-btn"
              onClick={handleCreate}
              disabled={saving || !goal.trim()}
            >
              <Plus size={14} />
              {saving ? "Creating..." : "Create mission"}
            </button>
          </div>
        </div>

        {activeMemoryItems.length > 0 && (
          <div className="assistant-memory-strip assistant-memory-strip-missions">
            <span className="assistant-memory-strip-label">Guided by memory</span>
            {activeMemoryItems.slice(0, 4).map((item) => (
              <span key={item.id} className="assistant-memory-chip">{item.label}</span>
            ))}
          </div>
        )}

        <div className="missions-body">
          {loading ? (
            <div className="missions-empty">Loading missions...</div>
          ) : missions.length === 0 ? (
            <div className="missions-empty">
              No missions yet. Create one to define a goal and let the session work toward it automatically.
            </div>
          ) : (
            grouped.map(({ state: groupState, items }) =>
              items.length > 0 ? (
                <section key={groupState} className="missions-section">
                  <div className="missions-section-title" style={{ color: stateColor(groupState) }}>
                    {formatState(groupState)} ({items.length})
                  </div>
                  {items.map((mission) => {
                    const isExpanded = expandedId === mission.id;
                    return (
                      <div key={mission.id} className={`missions-item missions-item-${mission.state}`}>
                        <div className="missions-item-main">
                          <div className="missions-item-row">
                            <span
                              className="missions-item-state-dot"
                              style={{ background: stateColor(mission.state) }}
                            />
                            <span className="missions-item-goal">{mission.goal}</span>
                            <span className="missions-item-project">
                              {projects[mission.project_index]?.name ?? `Project ${mission.project_index}`}
                            </span>
                          </div>
                          <div className="missions-item-meta">
                            <span>
                              iter {mission.iteration}/{mission.max_iterations === 0 ? "\u221e" : mission.max_iterations}
                            </span>
                            {mission.last_verdict && (
                              <span>
                                last: {formatVerdict(mission.last_verdict)}
                              </span>
                            )}
                            <span>
                              {mission.session_id
                                ? `session ${mission.session_id.slice(0, 8)}`
                                : "no session"}
                            </span>
                            <span>updated {formatRelativeDate(mission.updated_at)}</span>
                          </div>
                          {mission.last_eval_summary && (
                            <div className="missions-item-eval-summary">
                              {mission.last_eval_summary}
                            </div>
                          )}
                          {(mission.eval_history?.length ?? 0) > 0 && (
                            <button
                              className="missions-item-expand"
                              onClick={() => setExpandedId(isExpanded ? null : mission.id)}
                            >
                              {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                              {isExpanded ? "Hide" : "Show"} evaluation history ({mission.eval_history!.length})
                            </button>
                          )}
                          {isExpanded && mission.eval_history && (
                            <div className="missions-eval-history">
                              {mission.eval_history.map((record, i) => (
                                <div key={i} className="missions-eval-record">
                                  <span className="missions-eval-iter">#{record.iteration}</span>
                                  <span className={`missions-eval-verdict missions-eval-verdict-${record.verdict}`}>
                                    {formatVerdict(record.verdict)}
                                  </span>
                                  <span className="missions-eval-summary">{record.summary}</span>
                                  {record.next_step && (
                                    <span className="missions-eval-next">Next: {record.next_step}</span>
                                  )}
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                        <div className="missions-item-actions">
                          {canPerformAction(mission.state, "start") && (
                            <button
                              className="missions-action-btn missions-action-start"
                              title="Start"
                              onClick={() => handleAction(mission, "start")}
                            >
                              <Play size={14} />
                            </button>
                          )}
                          {canPerformAction(mission.state, "pause") && (
                            <button
                              className="missions-action-btn missions-action-pause"
                              title="Pause"
                              onClick={() => handleAction(mission, "pause")}
                            >
                              <Pause size={14} />
                            </button>
                          )}
                          {canPerformAction(mission.state, "resume") && (
                            <button
                              className="missions-action-btn missions-action-resume"
                              title="Resume"
                              onClick={() => handleAction(mission, "resume")}
                            >
                              <RotateCcw size={14} />
                            </button>
                          )}
                          {canPerformAction(mission.state, "cancel") && (
                            <button
                              className="missions-action-btn missions-action-cancel"
                              title="Cancel"
                              onClick={() => handleAction(mission, "cancel")}
                            >
                              <Ban size={14} />
                            </button>
                          )}
                          <button
                            className="missions-delete-btn"
                            onClick={() => handleDelete(mission.id)}
                            aria-label={`Delete mission`}
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </section>
              ) : null
            )
          )}
        </div>
      </div>
    </div>
  );
}
