import React, { useCallback, useEffect, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Clock3, Pencil, Play, Plus, Save, Trash2, X } from "lucide-react";
import { createRoutine, deleteRoutine, fetchRoutines, runRoutine, updateRoutine } from "./api";
import type { Mission, RoutineAction, RoutineDefinition, RoutineRunRecord, RoutineTrigger } from "./api";
import { buildRoutineSummary } from "./dailySummary";

interface Props {
  onClose: () => void;
  missions: Mission[];
  activeSessionId: string | null;
  autonomyMode: "observe" | "nudge" | "continue" | "autonomous";
}

export function RoutinesModal({ onClose, missions, activeSessionId, autonomyMode }: Props) {
  const [routines, setRoutines] = useState<RoutineDefinition[]>([]);
  const [runs, setRuns] = useState<RoutineRunRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [name, setName] = useState("");
  const [trigger, setTrigger] = useState<RoutineTrigger>("manual");
  const [action, setAction] = useState<RoutineAction>("open_inbox");
  const [missionId, setMissionId] = useState<string>("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState("");
  const [editingTrigger, setEditingTrigger] = useState<RoutineTrigger>("manual");
  const [editingAction, setEditingAction] = useState<RoutineAction>("open_inbox");
  const [editingMissionId, setEditingMissionId] = useState<string>("");
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const load = useCallback(async () => {
    const resp = await fetchRoutines();
    setRoutines(resp.routines);
    setRuns(resp.runs);
    setLoading(false);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleCreate = useCallback(async () => {
    if (!name.trim()) return;
    const routine = await createRoutine({
      name: name.trim(),
      trigger,
      action,
      mission_id: missionId || null,
      session_id: trigger === "on_session_idle" ? activeSessionId : null,
    });
    setRoutines((prev) => [routine, ...prev]);
    setName("");
    setMissionId("");
    setTrigger("manual");
    setAction("open_inbox");
  }, [name, trigger, action, missionId, activeSessionId]);

  const handleDelete = useCallback(async (id: string) => {
    await deleteRoutine(id);
    setRoutines((prev) => prev.filter((routine) => routine.id !== id));
  }, []);

  const handleRun = useCallback(async (id: string) => {
    const routine = routines.find((entry) => entry.id === id);
    const run = await runRoutine(id, {
      summary: routine ? buildRoutineSummary(routine, { activeSessionId, missions }) : undefined,
    });
    setRuns((prev) => [run, ...prev]);
  }, [routines, activeSessionId, missions]);

  const startEdit = useCallback((routine: RoutineDefinition) => {
    setEditingId(routine.id);
    setEditingName(routine.name);
    setEditingTrigger(routine.trigger);
    setEditingAction(routine.action);
    setEditingMissionId(routine.mission_id || "");
  }, []);

  const cancelEdit = useCallback(() => {
    setEditingId(null);
    setEditingName("");
    setEditingTrigger("manual");
    setEditingAction("open_inbox");
    setEditingMissionId("");
  }, []);

  const handleSaveEdit = useCallback(async (routine: RoutineDefinition) => {
    if (!editingName.trim()) return;
    const updated = await updateRoutine(routine.id, {
      name: editingName.trim(),
      trigger: editingTrigger,
      action: editingAction,
      mission_id: editingMissionId || null,
      session_id: editingTrigger === "on_session_idle" ? activeSessionId : null,
    });
    setRoutines((prev) => prev.map((item) => (item.id === updated.id ? updated : item)));
    cancelEdit();
  }, [editingName, editingTrigger, editingAction, editingMissionId, activeSessionId, cancelEdit]);

  return (
    <div className="routines-overlay" onClick={onClose}>
      <div ref={modalRef} className="routines-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="routines-header">
          <div className="routines-header-left">
            <Clock3 size={16} />
            <h3>Routines</h3>
          </div>
          <button onClick={onClose} aria-label="Close routines">
            <X size={16} />
          </button>
        </div>

        <div className="routines-create">
          <input className="routines-input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Routine name" />
          <div className="routines-create-grid">
            <select className="routines-select" value={trigger} onChange={(e) => setTrigger(e.target.value as RoutineTrigger)}>
              <option value="manual">Manual</option>
              <option value="on_session_idle">On Session Idle</option>
              <option value="daily_summary">Daily Summary</option>
            </select>
            <select className="routines-select" value={action} onChange={(e) => setAction(e.target.value as RoutineAction)}>
              <option value="open_inbox">Open Inbox</option>
              <option value="open_activity_feed">Open Activity Feed</option>
              <option value="review_mission">Review Mission</option>
            </select>
          </div>
          <select className="routines-select" value={missionId} onChange={(e) => setMissionId(e.target.value)}>
            <option value="">No mission link</option>
            {missions.map((mission) => (
              <option key={mission.id} value={mission.id}>{mission.title}</option>
            ))}
          </select>
          <div className="routines-create-footer">
            <span className="routines-context">Autonomy: {autonomyMode}</span>
            <button className="routines-create-btn" onClick={handleCreate} disabled={!name.trim()}>
              <Plus size={14} /> Save routine
            </button>
          </div>
        </div>

        <div className="routines-body">
          {loading ? (
            <div className="routines-empty">Loading routines...</div>
          ) : (
            <>
              <section className="routines-section">
                <div className="routines-section-title">Definitions</div>
                {routines.length === 0 ? (
                  <div className="routines-empty">No routines yet.</div>
                ) : (
                  routines.map((routine) => (
                    <div key={routine.id} className="routines-item">
                      <div className="routines-item-main">
                        {editingId === routine.id ? (
                          <>
                            <input
                              className="routines-input"
                              value={editingName}
                              onChange={(e) => setEditingName(e.target.value)}
                            />
                            <div className="routines-create-grid">
                              <select
                                className="routines-select"
                                value={editingTrigger}
                                onChange={(e) => setEditingTrigger(e.target.value as RoutineTrigger)}
                              >
                                <option value="manual">Manual</option>
                                <option value="on_session_idle">On Session Idle</option>
                                <option value="daily_summary">Daily Summary</option>
                              </select>
                              <select
                                className="routines-select"
                                value={editingAction}
                                onChange={(e) => setEditingAction(e.target.value as RoutineAction)}
                              >
                                <option value="open_inbox">Open Inbox</option>
                                <option value="open_activity_feed">Open Activity Feed</option>
                                <option value="review_mission">Review Mission</option>
                              </select>
                            </div>
                            <select
                              className="routines-select"
                              value={editingMissionId}
                              onChange={(e) => setEditingMissionId(e.target.value)}
                            >
                              <option value="">No mission link</option>
                              {missions.map((mission) => (
                                <option key={mission.id} value={mission.id}>{mission.title}</option>
                              ))}
                            </select>
                          </>
                        ) : (
                          <div className="routines-item-name">{routine.name}</div>
                        )}
                        <div className="routines-item-meta">{routine.trigger} • {routine.action}</div>
                      </div>
                      <div className="routines-item-actions">
                        {editingId === routine.id ? (
                          <>
                            <button onClick={() => handleSaveEdit(routine)}><Save size={14} /> Save</button>
                            <button onClick={cancelEdit}><X size={14} /> Cancel</button>
                          </>
                        ) : (
                          <button onClick={() => startEdit(routine)}><Pencil size={14} /> Edit</button>
                        )}
                        <button onClick={() => handleRun(routine.id)}><Play size={14} /> Run</button>
                        <button className="routines-delete-btn" onClick={() => handleDelete(routine.id)}><Trash2 size={14} /></button>
                      </div>
                    </div>
                  ))
                )}
              </section>
              <section className="routines-section">
                <div className="routines-section-title">Recent Runs</div>
                {runs.length === 0 ? (
                  <div className="routines-empty">No routine runs yet.</div>
                ) : (
                  runs.slice(0, 8).map((run) => (
                    <div key={run.id} className="routines-run-item">
                      <span>{run.summary}</span>
                      <span>{new Date(run.created_at).toLocaleTimeString()}</span>
                    </div>
                  ))
                )}
              </section>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
