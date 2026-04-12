import React, { useCallback, useEffect, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { BriefcaseBusiness, ExternalLink, Plus, Trash2, X } from "lucide-react";
import { createDelegatedWork, deleteDelegatedWork, fetchDelegatedWork, updateDelegatedWork } from "./api";
import type { DelegatedWorkItem, DelegationStatus, Mission } from "./api";

interface Props {
  onClose: () => void;
  missions: Mission[];
  activeSessionId: string | null;
  onOpenSession?: (sessionId: string) => void;
}

export function DelegationBoardModal({ onClose, missions, activeSessionId, onOpenSession }: Props) {
  const [items, setItems] = useState<DelegatedWorkItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [title, setTitle] = useState("");
  const [assignee, setAssignee] = useState("build");
  const [scope, setScope] = useState("");
  const [missionId, setMissionId] = useState("");
  const [subagentSessionId, setSubagentSessionId] = useState("");
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const load = useCallback(async () => {
    const resp = await fetchDelegatedWork();
    setItems(resp.items);
    setLoading(false);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleCreate = useCallback(async () => {
    if (!title.trim() || !scope.trim()) return;
    const item = await createDelegatedWork({
      title: title.trim(),
      assignee: assignee.trim(),
      scope: scope.trim(),
      mission_id: missionId || null,
      session_id: activeSessionId,
      subagent_session_id: subagentSessionId || null,
    });
    setItems((prev) => [item, ...prev]);
    setTitle("");
    setScope("");
    setMissionId("");
    setSubagentSessionId("");
  }, [title, assignee, scope, missionId, activeSessionId, subagentSessionId]);

  const handleDelete = useCallback(async (id: string) => {
    await deleteDelegatedWork(id);
    setItems((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const handleStatusChange = useCallback(async (item: DelegatedWorkItem, status: DelegationStatus) => {
    const updated = await updateDelegatedWork(item.id, { status });
    setItems((prev) => prev.map((entry) => (entry.id === updated.id ? updated : entry)));
  }, []);

  return (
    <div className="delegation-overlay" onClick={onClose}>
      <div ref={modalRef} className="delegation-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="delegation-header">
          <div className="delegation-header-left">
            <BriefcaseBusiness size={16} />
            <h3>Delegation Board</h3>
          </div>
          <button onClick={onClose} aria-label="Close delegation board">
            <X size={16} />
          </button>
        </div>

        <div className="delegation-scrollable">
        <div className="delegation-create">
          <input className="delegation-input" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Delegated task title" />
          <input className="delegation-input" value={assignee} onChange={(e) => setAssignee(e.target.value)} placeholder="Assignee" />
          <textarea className="delegation-textarea" value={scope} onChange={(e) => setScope(e.target.value)} rows={3} placeholder="Bounded task scope" />
          <div className="delegation-create-grid">
            <select className="delegation-select" value={missionId} onChange={(e) => setMissionId(e.target.value)}>
              <option value="">No mission link</option>
              {missions.map((mission) => (
                <option key={mission.id} value={mission.id}>{mission.goal}</option>
              ))}
            </select>
            <input className="delegation-input" value={subagentSessionId} onChange={(e) => setSubagentSessionId(e.target.value)} placeholder="Subagent session ID (optional)" />
          </div>
          <div className="delegation-create-footer">
            <span className="delegation-context">Current session: {activeSessionId ? activeSessionId.slice(0, 8) : "none"}</span>
            <button className="delegation-create-btn" onClick={handleCreate} disabled={!title.trim() || !scope.trim()}>
              <Plus size={14} /> Add delegated work
            </button>
          </div>
        </div>

        <div className="delegation-body">
          {loading ? (
            <div className="delegation-empty">Loading delegation board...</div>
          ) : items.length === 0 ? (
            <div className="delegation-empty">No delegated work yet.</div>
          ) : (
            items.map((item) => (
              <div key={item.id} className="delegation-item">
                <div className="delegation-item-main">
                  <div className="delegation-item-title">{item.title}</div>
                  <div className="delegation-item-scope">{item.scope}</div>
                  <div className="delegation-item-meta">
                    <span>{item.assignee}</span>
                    <span>{item.status}</span>
                    {item.subagent_session_id && <span>subagent {item.subagent_session_id}</span>}
                  </div>
                  <div className="delegation-status-actions">
                    {(["planned", "running", "completed"] as DelegationStatus[]).map((status) => (
                      <button
                        key={status}
                        className={`delegation-status-btn ${item.status === status ? "active" : ""}`}
                        onClick={() => handleStatusChange(item, status)}
                      >
                        {status}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="delegation-item-actions">
                  {item.subagent_session_id && onOpenSession && (
                    <button
                      className="delegation-open-btn"
                      onClick={() => onOpenSession(item.subagent_session_id!)}
                      title="Open linked output"
                    >
                      <ExternalLink size={14} />
                    </button>
                  )}
                  {item.session_id && onOpenSession && !item.subagent_session_id && (
                    <button
                      className="delegation-open-btn"
                      onClick={() => onOpenSession(item.session_id!)}
                      title="Open linked session"
                    >
                      <ExternalLink size={14} />
                    </button>
                  )}
                  <button className="delegation-delete-btn" onClick={() => handleDelete(item.id)}>
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
        </div>
      </div>
    </div>
  );
}
