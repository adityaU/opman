import React, { useState, useEffect, useCallback, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { fetchSessionTodos } from "./api";
import type { TodoItem } from "./types";
import { CheckSquare, Circle, Clock, XCircle, Loader2, X, ArrowUp, ArrowRight, ArrowDown } from "lucide-react";

interface Props {
  onClose: () => void;
  sessionId: string;
}

/** Status icon component */
function StatusIcon({ status }: { status: string }) {
  switch (status) {
    case "completed":
      return <CheckSquare size={13} className="todo-icon todo-icon-completed" />;
    case "in_progress":
      return <Clock size={13} className="todo-icon todo-icon-progress" />;
    case "cancelled":
      return <XCircle size={13} className="todo-icon todo-icon-cancelled" />;
    default:
      return <Circle size={13} className="todo-icon todo-icon-pending" />;
  }
}

/** Priority icon component */
function PriorityIcon({ priority }: { priority?: string }) {
  switch (priority) {
    case "high":
      return <ArrowUp size={11} className="todo-priority todo-priority-high" />;
    case "medium":
      return <ArrowRight size={11} className="todo-priority todo-priority-medium" />;
    case "low":
      return <ArrowDown size={11} className="todo-priority todo-priority-low" />;
    default:
      return <span className="todo-priority todo-priority-none">&middot;</span>;
  }
}

export function TodoPanelModal({ onClose, sessionId }: Props) {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
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

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, todos.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, 0));
      }
    },
    [todos.length]
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
          ) : todos.length === 0 ? (
            <div className="todo-panel-empty">No todos for this session</div>
          ) : (
            todos.map((todo, idx) => (
              <div
                key={todo.id}
                className={`todo-panel-item ${idx === selectedIdx ? "selected" : ""} todo-status-${todo.status}`}
                onClick={() => setSelectedIdx(idx)}
              >
                <StatusIcon status={todo.status} />
                <PriorityIcon priority={todo.priority} />
                <span className="todo-panel-content">{todo.content}</span>
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="todo-panel-footer">
          <kbd>Up/Down</kbd> Navigate
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
