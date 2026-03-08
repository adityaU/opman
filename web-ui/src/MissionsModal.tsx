import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Target, X, Plus, Trash2 } from "lucide-react";
import {
  createMission,
  deleteMission,
  fetchMissions,
  updateMission,
} from "./api";
import type { Mission, MissionStatus, PersonalMemoryItem, ProjectInfo } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { ActivityEvent } from "./api";
import { buildMissionHandoff } from "./handoffs";

interface Props {
  onClose: () => void;
  projects: ProjectInfo[];
  activeProjectIndex: number;
  activeSessionId: string | null;
  permissions?: PermissionRequest[];
  questions?: QuestionRequest[];
  activityEvents?: ActivityEvent[];
  onOpenInbox?: () => void;
  activeMemoryItems?: PersonalMemoryItem[];
  onOpenMissionSource?: (missionId: string) => void;
}

const STATUS_OPTIONS: MissionStatus[] = ["planned", "active", "blocked", "completed"];

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
}: Props) {
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

function MissionHandoffCard({
  mission,
  permissions,
  questions,
  activityEvents,
  onOpenInbox,
  onOpenMissionSource,
}: {
  mission: Mission;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
  onOpenInbox?: () => void;
  onOpenMissionSource?: (missionId: string) => void;
}) {
  const brief = buildMissionHandoff({
    mission,
    permissions,
    questions,
    activityEvents,
  });

  return (
    <div className="mission-handoff">
      <div className="mission-handoff-title">Resume Brief</div>
      <div className="mission-handoff-summary">{brief.summary}</div>
      {brief.blockers.length > 0 && (
        <div className="mission-handoff-blockers">
          <span className="mission-handoff-label">Blockers</span>
          {brief.blockers.map((blocker) => (
            <span key={blocker} className="mission-handoff-chip mission-handoff-chip-blocker">
              {blocker}
            </span>
          ))}
        </div>
      )}
      <div className="mission-handoff-recent">
        <span className="mission-handoff-label">Recent</span>
        {brief.recentChanges.map((change) => (
          <span key={change} className="mission-handoff-chip">{change}</span>
        ))}
      </div>
      <div className="mission-handoff-footer">
        <span className="mission-handoff-next">Next: {brief.nextAction}</span>
        {brief.blockers.length > 0 && onOpenInbox && (
          <button className="mission-handoff-btn" onClick={onOpenInbox}>
            Open inbox
          </button>
        )}
      </div>
      {brief.links.length > 0 && (
        <div className="mission-handoff-links">
          {brief.links.map((link) => (
            <button
              key={`${link.kind}:${link.sourceId ?? link.label}`}
              className="mission-handoff-link"
              onClick={() => {
                if (link.kind === "mission" && link.sourceId && onOpenMissionSource) {
                  onOpenMissionSource(link.sourceId);
                  return;
                }
                if ((link.kind === "permission" || link.kind === "question") && onOpenInbox) {
                  onOpenInbox();
                }
              }}
            >
              {link.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function formatStatus(status: MissionStatus): string {
  switch (status) {
    case "planned":
      return "Planned";
    case "active":
      return "Active";
    case "blocked":
      return "Blocked";
    case "completed":
      return "Completed";
  }
}

function formatRelativeDate(iso: string): string {
  try {
    const date = new Date(iso);
    return date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}
