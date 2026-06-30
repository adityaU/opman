import React, { useState, useRef, useCallback, useEffect } from "react";
import { X, Trash2, MessageSquare, Archive, RotateCcw } from "lucide-react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { TaskContentEditor } from "./TaskContentEditor";
import {
  createTask,
  patchTask,
  deleteTask as apiDeleteTask,
  setTaskArchived,
  type Board,
  type Task,
  type Priority,
} from "../api/kanban";

interface Props {
  board: Board;
  /** The task being edited, or null to create a new one. */
  task: Task | null;
  /** Lane to pre-select when creating. Defaults to the first lane. */
  defaultLaneId?: string;
  onClose: () => void;
  onSaved: (task: Task) => void;
  onDeleted: (taskId: string) => void;
  onError: (msg: string) => void;
  /** Open the chat session this task launched (only shown when one exists). */
  onOpenSession?: (sessionId: string) => void;
}

const PRIORITIES: Priority[] = ["low", "normal", "high", "urgent"];

export const TaskEditorModal: React.FC<Props> = function TaskEditorModal(p) {
  const modalRef = useRef<HTMLDivElement>(null);
  useEscape(p.onClose);
  useFocusTrap(modalRef);

  const editing = p.task;
  const [title, setTitle] = useState(editing?.title ?? "");
  const [description, setDescription] = useState(editing?.description ?? "");
  const [tags, setTags] = useState<string[]>(editing?.tags ?? []);
  const [tagInput, setTagInput] = useState("");
  const [priority, setPriority] = useState<Priority>(editing?.priority ?? "normal");
  const [laneId, setLaneId] = useState(
    editing?.lane_id ?? p.defaultLaneId ?? p.board.lanes[0]?.id ?? "",
  );
  const [saving, setSaving] = useState(false);
  // Track the created task id so attachments work right after the first save.
  const [savedTaskId, setSavedTaskId] = useState<string | null>(editing?.id ?? null);

  useEffect(() => {
    setSavedTaskId(editing?.id ?? null);
  }, [editing?.id]);

  const addTag = useCallback(() => {
    const t = tagInput.trim();
    if (t && !tags.includes(t)) setTags((prev) => [...prev, t]);
    setTagInput("");
  }, [tagInput, tags]);

  const removeTag = useCallback((t: string) => {
    setTags((prev) => prev.filter((x) => x !== t));
  }, []);

  const handleTagKey = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === ",") {
        e.preventDefault();
        addTag();
      } else if (e.key === "Backspace" && !tagInput && tags.length) {
        setTags((prev) => prev.slice(0, -1));
      }
    },
    [addTag, tagInput, tags.length],
  );

  const handleSave = useCallback(async () => {
    if (!title.trim()) {
      p.onError("Title is required.");
      return;
    }
    setSaving(true);
    try {
      let result: Task;
      if (savedTaskId) {
        result = await patchTask(savedTaskId, { title, description, tags, priority, lane_id: laneId });
      } else {
        result = await createTask({
          board_id: p.board.id,
          lane_id: laneId,
          title,
          description,
          tags,
          priority,
        });
        setSavedTaskId(result.id);
      }
      p.onSaved(result);
      p.onClose();
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to save task");
    } finally {
      setSaving(false);
    }
  }, [savedTaskId, title, description, tags, priority, laneId, p]);

  const handleArchive = useCallback(async () => {
    if (!savedTaskId) return;
    setSaving(true);
    try {
      const result = await setTaskArchived(savedTaskId, !editing?.archived);
      p.onSaved(result);
      p.onClose();
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to archive task");
      setSaving(false);
    }
  }, [savedTaskId, editing?.archived, p]);

  const handleDelete = useCallback(async () => {
    if (!savedTaskId) {
      p.onClose();
      return;
    }
    setSaving(true);
    try {
      await apiDeleteTask(savedTaskId);
      p.onDeleted(savedTaskId);
      p.onClose();
    } catch (e) {
      p.onError(e instanceof Error ? e.message : "Failed to delete task");
      setSaving(false);
    }
  }, [savedTaskId, p]);

  return (
    <div className="kanban-modal-overlay" onClick={p.onClose}>
      <div
        ref={modalRef}
        className="kanban-modal liquid-glass"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="kanban-modal-header">
          <h3>{editing ? "Edit task" : "New task"}</h3>
          {editing?.session_id && p.onOpenSession && (
            <button
              className="kanban-btn kanban-open-session-btn"
              onClick={() => {
                p.onOpenSession!(editing.session_id!);
                p.onClose();
              }}
              title="Open the chat session launched from this task"
            >
              <MessageSquare size={13} /> Open session
            </button>
          )}
          <button className="kanban-modal-close" onClick={p.onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>

        <div className="kanban-modal-body">
          <label className="kanban-field">
            <span className="kanban-field-label">Title</span>
            <input
              className="kanban-input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Task title"
              autoFocus
            />
          </label>

          <div className="kanban-field-row">
            <label className="kanban-field">
              <span className="kanban-field-label">Priority</span>
              <select
                className="kanban-input"
                value={priority}
                onChange={(e) => setPriority(e.target.value as Priority)}
              >
                {PRIORITIES.map((pr) => (
                  <option key={pr} value={pr}>
                    {pr.charAt(0).toUpperCase() + pr.slice(1)}
                  </option>
                ))}
              </select>
            </label>
            <label className="kanban-field">
              <span className="kanban-field-label">Lane</span>
              <select
                className="kanban-input"
                value={laneId}
                onChange={(e) => setLaneId(e.target.value)}
              >
                {p.board.lanes.map((lane) => (
                  <option key={lane.id} value={lane.id}>
                    {lane.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="kanban-field">
            <span className="kanban-field-label">Tags</span>
            <div className="kanban-tag-input-wrap">
              {tags.map((t) => (
                <span key={t} className="kanban-tag-chip kanban-tag-chip-removable">
                  {t}
                  <button type="button" onClick={() => removeTag(t)} aria-label={`Remove ${t}`}>
                    <X size={10} />
                  </button>
                </span>
              ))}
              <input
                className="kanban-tag-input"
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onKeyDown={handleTagKey}
                onBlur={addTag}
                placeholder="Add tag…"
              />
            </div>
          </div>

          <div className="kanban-field">
            <span className="kanban-field-label">Description</span>
            <TaskContentEditor
              value={description}
              onChange={setDescription}
              taskId={savedTaskId}
            />
          </div>
        </div>

        <div className="kanban-modal-footer">
          {editing && (
            <>
              <button className="kanban-btn kanban-btn-danger" onClick={handleDelete} disabled={saving}>
                <Trash2 size={13} /> Delete
              </button>
              <button className="kanban-btn" onClick={handleArchive} disabled={saving}>
                {editing.archived ? <><RotateCcw size={13} /> Unarchive</> : <><Archive size={13} /> Archive</>}
              </button>
            </>
          )}
          <div className="kanban-modal-footer-right">
            <button className="kanban-btn" onClick={p.onClose} disabled={saving}>
              Cancel
            </button>
            <button className="kanban-btn kanban-btn-primary" onClick={handleSave} disabled={saving}>
              {saving ? "Saving…" : editing ? "Save" : "Create"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
