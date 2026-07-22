import React, { useEffect, useRef } from "react";
import { ListPlus, X, Trash2 } from "lucide-react";

// ── Queue pill (sits in the selector-chip row, beside "memories") ──

interface QueuePillProps {
  count: number;
  active: boolean;
  onClick: () => void;
}

export function QueuePill({ count, active, onClick }: QueuePillProps) {
  if (count <= 0) return null;
  return (
    <button
      className={`prompt-chip prompt-chip-queue${active ? " prompt-chip-queue-active" : ""}`}
      onClick={onClick}
      title="Queued follow-ups — sent together on the next turn"
      aria-label={`${count} queued ${count === 1 ? "message" : "messages"}`}
    >
      <ListPlus size={11} />
      <span className="prompt-chip-label">
        {count} queued
      </span>
    </button>
  );
}

// ── Queue panel (popover listing the queued follow-ups) ────────────

interface QueuePanelProps {
  queued: string[];
  onRemove: (index: number) => void;
  onClear: () => void;
  onClose: () => void;
}

export function QueuePanel({ queued, onRemove, onClear, onClose }: QueuePanelProps) {
  const ref = useRef<HTMLDivElement>(null);

  // Dismiss on outside click / Escape.
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown, true);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <div className="prompt-queue-panel" ref={ref}>
      <div className="prompt-queue-header">
        <span className="prompt-queue-title">
          Queued — sent together next turn
        </span>
        {queued.length > 0 && (
          <button className="prompt-queue-clear" onClick={onClear} title="Remove all queued messages">
            <Trash2 size={12} />
            <span>Clear all</span>
          </button>
        )}
      </div>
      {queued.length === 0 ? (
        <div className="prompt-queue-empty">No queued messages</div>
      ) : (
        <ul className="prompt-queue-list">
          {queued.map((msg, i) => (
            <li key={i} className="prompt-queue-item">
              <span className="prompt-queue-index">{i + 1}</span>
              <span className="prompt-queue-text" title={msg}>{msg}</span>
              <button
                className="prompt-queue-remove"
                onClick={() => onRemove(i)}
                title="Remove this queued message"
                aria-label={`Remove queued message ${i + 1}`}
              >
                <X size={13} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
