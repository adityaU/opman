import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Target, X, Plus, Trash2 } from "lucide-react";
import {
  createMission,
  deleteMission,
  fetchMissions,
  updateMission,
} from "../api";
import type { Mission, MissionStatus } from "../api";
import { MissionHandoffCard } from "./MissionHandoffCard";
import { formatStatus, formatRelativeDate } from "./helpers";
import { STATUS_OPTIONS } from "./types";
import type { MissionsModalProps } from "./types";

export function MissionsModal({
  onClose,
  projects,
  activeProjectIndex,
  activeSessionId,
  permissions = [],
  questions = [],
  activityEvents = [],
  onOpenInbox,
  activeMemoryItems = [],
  onOpenMissionSource,
}: MissionsModalProps) {
  const [missions, setMissions] = useState<Mission[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [title, setTitle] = useState("");
  const [goal, setGoal] = useState("");
  const [nextAction, setNextAction] = useState("");
  const [status, setStatus] = useState<MissionStatus>("planned");
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
    const order: MissionStatus[] = ["active", "blocked", "planned", "completed"];
    return order.map((current) => ({
      status: current,
      items: missions.filter((mission) => mission.status === current),
    }));
  }, [missions]);

  const handleCreate = useCallback(async () => {
    const trimmedTitle = title.trim();
    const trimmedGoal = goal.trim();
    if (!trimmedTitle || !trimmedGoal) return;

    setSaving(true);
    try {
      await createMission({
        title: trimmedTitle,
        goal: trimmedGoal,
        next_action: nextAction.trim(),
        status,
        project_index: activeProjectIndex,
        session_id: activeSessionId,
      });
      setTitle("");
      setGoal("");
      setNextAction("");
      setStatus("planned");
      await loadMissions();
    } finally {
      setSaving(false);
    }
  }, [title, goal, nextAction, status, activeProjectIndex, activeSessionId, loadMissions]);

  const handleStatusChange = useCallback(async (mission: Mission, nextStatus: MissionStatus) => {
    try {
      const updated = await updateMission(mission.id, { status: nextStatus });
      setMissions((prev) => prev.map((item) => (item.id === updated.id ? updated : item)));
    } catch {
      // ignore
    }
  }, []);

  const handleDelete = useCallback(async (missionId: string) => {
    try {
      await deleteMission(missionId);
      setMissions((prev) => prev.filter((mission) => mission.id !== missionId));
    } catch {
      // ignore
    }
  }, []);

  const handleNextActionBlur = useCallback(async (mission: Mission, value: string) => {
    const trimmed = value.trim();
    if (trimmed === mission.next_action) return;
    try {
      const updated = await updateMission(mission.id, { next_action: trimmed });
      setMissions((prev) => prev.map((item) => (item.id === updated.id ? updated : item)));
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
          <div className="missions-create-grid">
            <input
              className="missions-input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Mission title"
            />
            <select
              className="missions-select"
              value={status}
              onChange={(e) => setStatus(e.target.value as MissionStatus)}
            >
              {STATUS_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {formatStatus(option)}
                </option>
              ))}
            </select>
          </div>
          <textarea
            className="missions-textarea"
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            placeholder="What outcome are you trying to achieve?"
            rows={3}
          />
          <input
            className="missions-input"
            value={nextAction}
            onChange={(e) => setNextAction(e.target.value)}
            placeholder="Next action (optional)"
          />
          <div className="missions-create-footer">
            <span className="missions-context">
              {projectName}
              {activeSessionId ? ` • session ${activeSessionId.slice(0, 8)}` : " • no session linked"}
            </span>
            <button
              className="missions-create-btn"
              onClick={handleCreate}
              disabled={saving || !title.trim() || !goal.trim()}
            >
              <Plus size={14} />
              {saving ? "Saving..." : "Create mission"}
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
              No missions yet. Create one to track intent above individual sessions.
            </div>
          ) : (
            grouped.map(({ status: groupStatus, items }) =>
              items.length > 0 ? (
                <section key={groupStatus} className="missions-section">
                  <div className="missions-section-title">{formatStatus(groupStatus)}</div>
                  {items.map((mission) => (
                    <div key={mission.id} className={`missions-item missions-item-${mission.status}`}>
                      <div className="missions-item-main">
                        <div className="missions-item-row">
                          <span className="missions-item-title">{mission.title}</span>
                          <span className="missions-item-project">
                            {projects[mission.project_index]?.name ?? `Project ${mission.project_index}`}
                          </span>
                        </div>
                        <div className="missions-item-goal">{mission.goal}</div>
                        <input
                          className="missions-item-next"
                          defaultValue={mission.next_action}
                          placeholder="Add next action"
                          onBlur={(e) => handleNextActionBlur(mission, e.target.value)}
                        />
                        <div className="missions-item-meta">
                          <span>
                            {mission.session_id
                              ? `linked session ${mission.session_id.slice(0, 8)}`
                              : "no linked session"}
                          </span>
                          <span>updated {formatRelativeDate(mission.updated_at)}</span>
                        </div>
                        <MissionHandoffCard
                          mission={mission}
                          permissions={permissions}
                          questions={questions}
                          activityEvents={activityEvents}
                          onOpenInbox={onOpenInbox}
                          onOpenMissionSource={onOpenMissionSource}
                        />
                      </div>
                      <div className="missions-item-actions">
                        <select
                          className="missions-select missions-item-status"
                          value={mission.status}
                          onChange={(e) =>
                            handleStatusChange(mission, e.target.value as MissionStatus)
                          }
                        >
                          {STATUS_OPTIONS.map((option) => (
                            <option key={option} value={option}>
                              {formatStatus(option)}
                            </option>
                          ))}
                        </select>
                        <button
                          className="missions-delete-btn"
                          onClick={() => handleDelete(mission.id)}
                          aria-label={`Delete ${mission.title}`}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                  ))}
                </section>
              ) : null
            )
          )}
        </div>
      </div>
    </div>
  );
}
