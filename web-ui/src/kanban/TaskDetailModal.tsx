import React, { useState, useRef, useCallback, useEffect, useMemo } from "react";
import { X, MessageSquare, Play, Pencil, Send, Paperclip, Square } from "lucide-react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { NoteRow } from "./TaskNoteRow";
import {
  fetchTaskDetail,
  addUserNote,
  abortTask,
  assetUrl,
  type Board,
  type Task,
  type TaskDetail,
  type PipelineRun,
} from "../api/kanban";

interface Props {
  board: Board;
  taskId: string;
  /** Pipeline run for this task, if it was launched in pipeline mode. */
  pipeline?: PipelineRun;
  onClose: () => void;
  onEdit: (task: Task) => void;
  onLaunch: (task: Task) => void;
  onOpenSession: (sessionId: string) => void;
  onError: (msg: string) => void;
}

const STAGE_LABEL: Record<string, string> = {
  pending: "Pending",
  running: "Running",
  done: "Done",
  failed: "Failed",
};

export const TaskDetailModal: React.FC<Props> = function TaskDetailModal(p) {
  const modalRef = useRef<HTMLDivElement>(null);
  useEscape(p.onClose);
  useFocusTrap(modalRef);

  const [detail, setDetail] = useState<TaskDetail | null>(null);
  const [noteDraft, setNoteDraft] = useState("");
  const [sending, setSending] = useState(false);

  const laneName = useCallback(
    (id: string | null) => (id ? p.board.lanes.find((l) => l.id === id)?.name ?? id : ""),
    [p.board.lanes],
  );

  const load = useCallback(async () => {
    try {
      setDetail(await fetchTaskDetail(p.taskId));
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to load task details");
    }
  }, [p]);

  useEffect(() => {
    load();
  }, [load]);

  // Live-refresh notes/stages as the agent reports progress on this task.
  useEffect(() => {
    const es = new EventSource("/api/events", { withCredentials: true });
    const onTask = (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data) as { task_id?: string };
        if (data.task_id === p.taskId) load();
      } catch {
        /* ignore */
      }
    };
    es.addEventListener("kanban_task", onTask as EventListener);
    return () => {
      es.removeEventListener("kanban_task", onTask as EventListener);
      es.close();
    };
  }, [p.taskId, load]);

  const handleSend = useCallback(async () => {
    const body = noteDraft.trim();
    if (!body) return;
    setSending(true);
    try {
      const note = await addUserNote(p.taskId, body);
      setNoteDraft("");
      setDetail((d) => (d ? { ...d, notes: [...d.notes, note] } : d));
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to add note");
    } finally {
      setSending(false);
    }
  }, [noteDraft, p]);

  const handleNoteKey = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const isBusy = detail?.run_state === "running" || detail?.run_state === "launching";
  const notes = useMemo(() => detail?.notes ?? [], [detail]);

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
          <h3>{detail?.title ?? "Task"}</h3>
          {detail && (
            <button
              className="kanban-btn"
              onClick={() => {
                p.onEdit(detail);
              }}
              title="Edit this task"
            >
              <Pencil size={13} /> Edit
            </button>
          )}
          {detail?.session_id && (
            <button
              className="kanban-btn kanban-open-session-btn"
              onClick={() => p.onOpenSession(detail.session_id!)}
              title="Open the chat session for this task"
            >
              <MessageSquare size={13} /> Open session
            </button>
          )}
          <button className="kanban-modal-close" onClick={p.onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>

        <div className="kanban-modal-body kanban-detail-body">
          {/* ── Meta row ── */}
          <div className="kanban-detail-meta">
            <span className={`kanban-priority-chip kanban-priority-${detail?.priority ?? "normal"}`}>
              {detail?.priority ?? "normal"}
            </span>
            <span className="kanban-detail-lane">{laneName(detail?.lane_id ?? null)}</span>
            {detail && detail.run_state !== "idle" && (
              <span className={`kanban-runstate kanban-runstate-${detail.run_state}`}>
                {detail.run_state}
              </span>
            )}
            {(detail?.tags ?? []).map((t) => (
              <span key={t} className="kanban-tag-chip">
                {t}
              </span>
            ))}
            {detail && (detail.run_state === "idle" ? (
              <button className="kanban-launch-btn" onClick={() => p.onLaunch(detail)}>
                <Play size={11} /> Launch
              </button>
            ) : isBusy ? (
              <button
                className="kanban-btn kanban-btn-danger kanban-btn-sm"
                onClick={() => abortTask(p.taskId).then(load).catch(() => {})}
              >
                <Square size={11} /> Abort
              </button>
            ) : null)}
          </div>

          {detail?.description && (
            <div className="kanban-detail-desc">{detail.description}</div>
          )}

          {/* ── Pipeline stages ── */}
          {p.pipeline && (
            <div className="kanban-detail-section">
              <h4 className="kanban-detail-h">Pipeline</h4>
              <ol className="kanban-pipeline-stages kanban-pipeline-stages-vert">
                {p.pipeline.stages.map((s, i) => {
                  const active = i === p.pipeline!.current_index && p.pipeline!.status === "running";
                  return (
                    <li
                      key={`${s.lane_id}-${i}`}
                      className={`kanban-pipeline-stage kanban-stage-${s.status}${active ? " is-active" : ""}`}
                    >
                      <span className="kanban-pipeline-stage-idx">{i + 1}</span>
                      <span className="kanban-pipeline-stage-name">{laneName(s.lane_id)}</span>
                      <span className={`kanban-stage-badge kanban-stage-badge-${s.status}`}>
                        {STAGE_LABEL[s.status] ?? s.status}
                      </span>
                      {s.session_id && (
                        <button
                          className="kanban-stage-open"
                          title="Open this stage's session"
                          onClick={() => p.onOpenSession(s.session_id!)}
                        >
                          <MessageSquare size={11} />
                        </button>
                      )}
                    </li>
                  );
                })}
              </ol>
            </div>
          )}

          {/* ── Attachments ── */}
          {detail && detail.attachments.length > 0 && (
            <div className="kanban-detail-section">
              <h4 className="kanban-detail-h">
                <Paperclip size={12} /> Attachments
              </h4>
              <div className="kanban-detail-attachments">
                {detail.attachments.map((a) =>
                  a.kind === "image" ? (
                    <a key={a.id} href={a.url} target="_blank" rel="noreferrer">
                      <img src={assetUrl(p.taskId, a.filename)} alt={a.filename} />
                    </a>
                  ) : (
                    <a key={a.id} href={a.url} target="_blank" rel="noreferrer" className="kanban-attach-file">
                      {a.filename}
                    </a>
                  ),
                )}
              </div>
            </div>
          )}

          {/* ── Notes timeline ── */}
          <div className="kanban-detail-section">
            <h4 className="kanban-detail-h">Activity &amp; notes</h4>
            {notes.length === 0 ? (
              <p className="kanban-detail-empty">No notes yet.</p>
            ) : (
              <ul className="kanban-notes">
                {notes.map((n) => (
                  <NoteRow key={n.id} note={n} laneName={laneName} />
                ))}
              </ul>
            )}
          </div>
        </div>

        <div className="kanban-modal-footer kanban-detail-footer">
          <div className="kanban-note-compose">
            <textarea
              className="kanban-input"
              value={noteDraft}
              onChange={(e) => setNoteDraft(e.target.value)}
              onKeyDown={handleNoteKey}
              placeholder={
                isBusy
                  ? "Add a note — delivered to the running agent…  (⌘/Ctrl+Enter)"
                  : "Add a note…  (⌘/Ctrl+Enter)"
              }
              rows={2}
            />
            <button
              className="kanban-btn kanban-btn-primary"
              onClick={handleSend}
              disabled={sending || !noteDraft.trim()}
              title="Add note"
            >
              <Send size={13} /> {sending ? "Sending…" : "Send"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
