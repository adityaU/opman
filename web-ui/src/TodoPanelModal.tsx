import React, { useState, useEffect, useCallback, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { fetchSessionTodos, updateSessionTodos } from "./api";
import type { TodoItem } from "./types";
import {
  CheckSquare, Circle, Clock, XCircle, Loader2, X,
  ArrowUp, ArrowRight, ArrowDown, Plus, Pencil, Trash2, Copy,
} from "lucide-react";

interface Props {
  onClose: () => void;
  sessionId: string;
}

/** Status icon component */
function StatusIcon({ status, onClick }: { status: string; onClick?: () => void }) {
  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onClick?.();
  };
  const cls = `todo-icon todo-icon-${status === "in_progress" ? "progress" : status} ${onClick ? "todo-icon-clickable" : ""}`;
  switch (status) {
    case "completed":
      return <CheckSquare size={13} className={cls} onClick={handleClick} />;
    case "in_progress":
      return <Clock size={13} className={cls} onClick={handleClick} />;
    case "cancelled":
      return <XCircle size={13} className={cls} onClick={handleClick} />;
    default:
      return <Circle size={13} className={cls} onClick={handleClick} />;
  }
}

/** Priority icon component */
function PriorityIcon({ priority, onClick }: { priority?: string; onClick?: () => void }) {
  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onClick?.();
  };
  const clickCls = onClick ? " todo-priority-clickable" : "";
  switch (priority) {
    case "high":
      return <ArrowUp size={11} className={`todo-priority todo-priority-high${clickCls}`} onClick={handleClick} />;
    case "medium":
      return <ArrowRight size={11} className={`todo-priority todo-priority-medium${clickCls}`} onClick={handleClick} />;
    case "low":
      return <ArrowDown size={11} className={`todo-priority todo-priority-low${clickCls}`} onClick={handleClick} />;
    default:
      return <span className={`todo-priority todo-priority-none${clickCls}`} onClick={handleClick}>&middot;</span>;
  }
}

/** Cycle to the next status */
function nextStatus(current: string): string {
  switch (current) {
    case "pending": return "in_progress";
    case "in_progress": return "completed";
    case "completed": return "pending";
    case "cancelled": return "pending";
    default: return "pending";
  }
}

/** Cycle to the next priority */
function nextPriority(current: string | undefined): string {
  switch (current) {
    case "low": return "medium";
    case "medium": return "high";
    case "high": return "low";
    default: return "low";
  }
}

/** Inline editing state */
interface EditingState {
  /** null = new todo, number = editing at index */
  index: number | null;
  content: string;
  priority: string;
}

export function TodoPanelModal({ onClose, sessionId }: Props) {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [editing, setEditing] = useState<EditingState | null>(null);
  const [copyFlash, setCopyFlash] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEscape(() => {
    if (editing) {
      setEditing(null);
    } else {
      onClose();
    }
  });
  useFocusTrap(modalRef);

  // Fetch todos on mount
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    fetchSessionTodos(sessionId)
      .then((data) => {
        if (!cancelled) {
          setTodos(Array.isArray(data) ? data : []);
          setLoading(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setError("Failed to load todos");
          setLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [sessionId]);

  // Listen for SSE todo.updated events to refetch
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.sessionID === sessionId) {
        fetchSessionTodos(sessionId)
          .then((data) => setTodos(Array.isArray(data) ? data : []))
          .catch(() => { /* ignore refetch errors */ });
      }
    };
    window.addEventListener("opman:todo-updated", handler);
    return () => window.removeEventListener("opman:todo-updated", handler);
  }, [sessionId]);

  // Focus edit input when editing starts
  useEffect(() => {
    if (editing && editInputRef.current) {
      editInputRef.current.focus();
    }
  }, [editing]);

  // Persist changes to backend (debounced)
  const persistTodos = useCallback(
    (updatedTodos: TodoItem[]) => {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = setTimeout(() => {
        const payload = updatedTodos.map((t) => ({
          content: t.content,
          status: t.status,
          priority: t.priority || "medium",
        }));
        updateSessionTodos(sessionId, payload).catch((err) => {
          console.error("Failed to save todos:", err);
        });
      }, 300);
    },
    [sessionId]
  );

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    };
  }, []);

  // Mutate helper: updates state and persists
  const mutateTodos = useCallback(
    (updater: (prev: TodoItem[]) => TodoItem[]) => {
      setTodos((prev) => {
        const next = updater(prev);
        persistTodos(next);
        return next;
      });
    },
    [persistTodos]
  );

  // ── Actions ───────────────────────────────────────────

  const toggleStatus = useCallback(
    (idx: number) => {
      mutateTodos((prev) =>
        prev.map((t, i) =>
          i === idx ? { ...t, status: nextStatus(t.status) as TodoItem["status"] } : t
        )
      );
    },
    [mutateTodos]
  );

  const cyclePriority = useCallback(
    (idx: number) => {
      mutateTodos((prev) =>
        prev.map((t, i) =>
          i === idx ? { ...t, priority: nextPriority(t.priority) } : t
        )
      );
    },
    [mutateTodos]
  );

  const deleteTodo = useCallback(
    (idx: number) => {
      mutateTodos((prev) => prev.filter((_, i) => i !== idx));
      setSelectedIdx((sel) => {
        const newLen = todos.length - 1;
        if (newLen <= 0) return 0;
        return sel >= newLen ? newLen - 1 : sel;
      });
    },
    [mutateTodos, todos.length]
  );

  const moveUp = useCallback(
    (idx: number) => {
      if (idx <= 0) return;
      mutateTodos((prev) => {
        const next = [...prev];
        [next[idx - 1], next[idx]] = [next[idx], next[idx - 1]];
        return next;
      });
      setSelectedIdx(idx - 1);
    },
    [mutateTodos]
  );

  const moveDown = useCallback(
    (idx: number) => {
      mutateTodos((prev) => {
        if (idx >= prev.length - 1) return prev;
        const next = [...prev];
        [next[idx], next[idx + 1]] = [next[idx + 1], next[idx]];
        return next;
      });
      setSelectedIdx((sel) => sel + 1);
    },
    [mutateTodos]
  );

  const copyContent = useCallback(
    (idx: number) => {
      const todo = todos[idx];
      if (!todo) return;
      navigator.clipboard.writeText(todo.content).then(() => {
        setCopyFlash(true);
        setTimeout(() => setCopyFlash(false), 800);
      }).catch(() => { /* ignore clipboard errors */ });
    },
    [todos]
  );

  const startCreate = useCallback(() => {
    setEditing({ index: null, content: "", priority: "medium" });
  }, []);

  const startEdit = useCallback(
    (idx: number) => {
      const todo = todos[idx];
      if (!todo) return;
      setEditing({ index: idx, content: todo.content, priority: todo.priority || "medium" });
    },
    [todos]
  );

  const commitEdit = useCallback(() => {
    if (!editing) return;
    const content = editing.content.trim();
    if (!content) {
      setEditing(null);
      return;
    }
    if (editing.index === null) {
      // Create new
      const newTodo: TodoItem = {
        id: `temp-${Date.now()}`,
        sessionID: sessionId,
        content,
        status: "pending",
        priority: editing.priority,
      };
      mutateTodos((prev) => [...prev, newTodo]);
      setSelectedIdx(todos.length); // select the new one
    } else {
      // Update existing
      const idx = editing.index;
      mutateTodos((prev) =>
        prev.map((t, i) =>
          i === idx ? { ...t, content, priority: editing.priority } : t
        )
      );
    }
    setEditing(null);
  }, [editing, mutateTodos, sessionId, todos.length]);

  // ── Keyboard handling ─────────────────────────────────

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // When editing, only handle edit-specific keys
      if (editing) {
        if (e.key === "Enter") {
          e.preventDefault();
          commitEdit();
        }
        // Esc handled by useEscape hook
        return;
      }

      switch (e.key) {
        case "ArrowDown":
        case "j":
          e.preventDefault();
          setSelectedIdx((i) => Math.min(i + 1, todos.length - 1));
          break;
        case "ArrowUp":
        case "k":
          e.preventDefault();
          setSelectedIdx((i) => Math.max(i - 1, 0));
          break;
        case " ": // Space: toggle status
          e.preventDefault();
          if (todos.length > 0) toggleStatus(selectedIdx);
          break;
        case "n": // New todo
          e.preventDefault();
          startCreate();
          break;
        case "e": // Edit selected
          e.preventDefault();
          if (todos.length > 0) startEdit(selectedIdx);
          break;
        case "d": // Delete selected
          e.preventDefault();
          if (todos.length > 0) deleteTodo(selectedIdx);
          break;
        case "p": // Cycle priority
          e.preventDefault();
          if (todos.length > 0) cyclePriority(selectedIdx);
          break;
        case "K": // Move up (Shift+K)
          e.preventDefault();
          moveUp(selectedIdx);
          break;
        case "J": // Move down (Shift+J)
          e.preventDefault();
          moveDown(selectedIdx);
          break;
        case "y": // Copy content
          e.preventDefault();
          if (todos.length > 0) copyContent(selectedIdx);
          break;
      }
    },
    [
      editing, todos.length, selectedIdx,
      commitEdit, toggleStatus, startCreate, startEdit, deleteTodo,
      cyclePriority, moveUp, moveDown, copyContent,
    ]
  );

  // Scroll selected into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIdx] as HTMLElement;
    if (item) item.scrollIntoView({ block: "nearest" });
  }, [selectedIdx]);

  // Summary counts
  const counts = {
    total: todos.length,
    completed: todos.filter((t) => t.status === "completed").length,
    inProgress: todos.filter((t) => t.status === "in_progress").length,
    pending: todos.filter((t) => t.status === "pending").length,
    cancelled: todos.filter((t) => t.status === "cancelled").length,
  };

  const shortId = sessionId.length > 12 ? sessionId.slice(0, 12) : sessionId;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="todo-panel-modal"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={0}
        role="dialog"
        aria-modal="true"
        aria-label="Todo list"
        ref={modalRef}
      >
        {/* Header */}
        <div className="todo-panel-header">
          <CheckSquare size={14} />
          <span>Todos</span>
          <span className="todo-panel-session">{shortId}</span>
          {copyFlash && <span className="todo-copy-flash">Copied!</span>}
          <button
            className="todo-panel-action-btn"
            onClick={startCreate}
            title="New todo (n)"
            aria-label="New todo"
          >
            <Plus size={14} />
          </button>
          <button className="todo-panel-close" onClick={onClose} aria-label="Close todo panel">
            <X size={14} />
          </button>
        </div>

        {/* Status summary */}
        {!loading && !error && todos.length > 0 && (
          <div className="todo-panel-summary">
            <span className="todo-summary-item">{counts.total} total</span>
            <span className="todo-summary-item todo-summary-completed">{counts.completed} done</span>
            <span className="todo-summary-item todo-summary-progress">{counts.inProgress} active</span>
            <span className="todo-summary-item todo-summary-pending">{counts.pending} pending</span>
            {counts.cancelled > 0 && (
              <span className="todo-summary-item todo-summary-cancelled">{counts.cancelled} cancelled</span>
            )}
          </div>
        )}

        {/* Todo list */}
        <div className="todo-panel-list" ref={listRef}>
          {loading ? (
            <div className="todo-panel-empty">
              <Loader2 size={16} className="spinning" />
              <span>Loading todos...</span>
            </div>
          ) : error ? (
            <div className="todo-panel-empty todo-panel-error">{error}</div>
          ) : todos.length === 0 && !editing ? (
            <div className="todo-panel-empty">
              No todos for this session
              <button className="todo-panel-create-btn" onClick={startCreate}>
                <Plus size={12} /> Create one
              </button>
            </div>
          ) : (
            <>
              {todos.map((todo, idx) => {
                const isEditing = editing && editing.index === idx;
                return (
                  <div
                    key={todo.id || idx}
                    className={`todo-panel-item ${idx === selectedIdx ? "selected" : ""} todo-status-${todo.status}`}
                    onClick={() => setSelectedIdx(idx)}
                    onDoubleClick={() => startEdit(idx)}
                  >
                    <StatusIcon
                      status={todo.status}
                      onClick={() => toggleStatus(idx)}
                    />
                    <PriorityIcon
                      priority={todo.priority}
                      onClick={() => cyclePriority(idx)}
                    />
                    {isEditing ? (
                      <input
                        ref={editInputRef}
                        className="todo-edit-input"
                        value={editing.content}
                        onChange={(e) => setEditing({ ...editing, content: e.target.value })}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            e.stopPropagation();
                            commitEdit();
                          } else if (e.key === "Escape") {
                            e.preventDefault();
                            e.stopPropagation();
                            setEditing(null);
                          }
                        }}
                        onBlur={commitEdit}
                        placeholder="Todo content..."
                      />
                    ) : (
                      <span className="todo-panel-content">{todo.content}</span>
                    )}
                    {/* Action buttons visible on hover / selected */}
                    {!isEditing && (
                      <div className="todo-item-actions">
                        <button
                          className="todo-action-btn"
                          onClick={(e) => { e.stopPropagation(); startEdit(idx); }}
                          title="Edit (e)"
                        >
                          <Pencil size={11} />
                        </button>
                        <button
                          className="todo-action-btn"
                          onClick={(e) => { e.stopPropagation(); copyContent(idx); }}
                          title="Copy (y)"
                        >
                          <Copy size={11} />
                        </button>
                        <button
                          className="todo-action-btn todo-action-btn-danger"
                          onClick={(e) => { e.stopPropagation(); deleteTodo(idx); }}
                          title="Delete (d)"
                        >
                          <Trash2 size={11} />
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}

              {/* Inline create row */}
              {editing && editing.index === null && (
                <div className="todo-panel-item todo-panel-item-editing selected">
                  <Circle size={13} className="todo-icon todo-icon-pending" />
                  <PriorityIcon
                    priority={editing.priority}
                    onClick={() => setEditing({ ...editing, priority: nextPriority(editing.priority) })}
                  />
                  <input
                    ref={editInputRef}
                    className="todo-edit-input"
                    value={editing.content}
                    onChange={(e) => setEditing({ ...editing, content: e.target.value })}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        e.stopPropagation();
                        commitEdit();
                      } else if (e.key === "Escape") {
                        e.preventDefault();
                        e.stopPropagation();
                        setEditing(null);
                      }
                    }}
                    onBlur={commitEdit}
                    placeholder="New todo..."
                  />
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="todo-panel-footer">
          <kbd>Space</kbd> Toggle
          <kbd>n</kbd> New
          <kbd>e</kbd> Edit
          <kbd>d</kbd> Delete
          <kbd>p</kbd> Priority
          <kbd>K/J</kbd> Reorder
          <kbd>y</kbd> Copy
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
