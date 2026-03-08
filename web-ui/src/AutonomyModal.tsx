import React, { useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Bot, X } from "lucide-react";
import type { AutonomyMode } from "./api";

interface Props {
  onClose: () => void;
  mode: AutonomyMode;
  onChange: (mode: AutonomyMode) => void;
}

const MODES: Array<{ id: AutonomyMode; label: string; description: string }> = [
  { id: "observe", label: "Observe", description: "Only react when directly asked." },
  { id: "nudge", label: "Nudge", description: "Surface gentle reminders and summaries." },
  { id: "continue", label: "Continue", description: "Allow limited proactive continuation flows." },
  { id: "autonomous", label: "Autonomous", description: "Allow the highest available proactive behavior." },
];

export function AutonomyModal({ onClose, mode, onChange }: Props) {
  const modalRef = useRef<HTMLDivElement>(null);
  useEscape(onClose);
  useFocusTrap(modalRef);

  return (
    <div className="autonomy-overlay" onClick={onClose}>
      <div ref={modalRef} className="autonomy-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="autonomy-header">
          <div className="autonomy-header-left">
            <Bot size={16} />
            <h3>Autonomy</h3>
          </div>
          <button onClick={onClose} aria-label="Close autonomy settings">
            <X size={16} />
          </button>
        </div>
        <div className="autonomy-body">
          <div className="assistant-center-briefing">
            <div className="assistant-center-briefing-title">Behavior Profile</div>
            <div className="assistant-center-briefing-summary">
              Choose how proactive opman should feel across reminders, summaries, and autonomous continuation.
            </div>
          </div>
          {MODES.map((entry) => (
            <button
              key={entry.id}
              className={`autonomy-item ${mode === entry.id ? "active" : ""}`}
              onClick={() => onChange(entry.id)}
            >
              <div className="autonomy-item-label">{entry.label}</div>
              <div className="autonomy-item-desc">{entry.description}</div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
