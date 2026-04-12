/**
 * AutoOpenModal — configure which tool-call accordions auto-open by default.
 *
 * Reuses the notification-prefs-* CSS classes (same visual pattern).
 * Uses the `useAutoOpen` hook for localStorage persistence.
 */

import React, { useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import {
  useAutoOpen,
  TOOL_CATEGORIES,
  type ToolCategory,
} from "./hooks/useAutoOpen";

export interface AutoOpenModalProps {
  onClose: () => void;
}

export const AutoOpenModal: React.FC<AutoOpenModalProps> = ({ onClose }) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const { config, toggle } = useAutoOpen();

  useEscape(onClose);
  useFocusTrap(modalRef);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        ref={modalRef}
        className="notification-prefs-modal"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="notification-prefs-header">
          <h3>Auto Open</h3>
          <button onClick={onClose} aria-label="Close">
            <svg width={14} height={14} viewBox="0 0 24 24" fill="none"
              stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div className="notification-prefs-body">
          <div className="notification-prefs-permission">
            <div className="notification-prefs-permission-info">
              <span className="notification-prefs-permission-label">
                Choose which tool-call accordions expand automatically.
              </span>
            </div>
          </div>

          {TOOL_CATEGORIES.map((cat) => {
            const isOn = config[cat.key];
            return (
              <CategoryRow
                key={cat.key}
                cat={cat.key}
                label={cat.label}
                description={cat.description}
                iconPath={cat.iconPath}
                isOn={isOn}
                onToggle={toggle}
              />
            );
          })}
        </div>

        <div className="notification-prefs-footer">
          All toggles are OFF by default. Changes are saved immediately.
        </div>
      </div>
    </div>
  );
};

// ── Row sub-component ────────────────────────────────────────────

const CategoryRow: React.FC<{
  cat: ToolCategory;
  label: string;
  description: string;
  iconPath: string;
  isOn: boolean;
  onToggle: (cat: ToolCategory) => void;
}> = React.memo(({ cat, label, description, iconPath, isOn, onToggle }) => (
  <div className="notification-prefs-item" onClick={() => onToggle(cat)}>
    <div className="notification-prefs-item-left">
      <svg width={14} height={14} viewBox="0 0 24 24" fill="none"
        stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d={iconPath} />
      </svg>
      <div>
        <div className="notification-prefs-item-label">{label}</div>
        <div className="notification-prefs-item-desc">{description}</div>
      </div>
    </div>
    <span className={isOn ? "notification-prefs-badge on" : "notification-prefs-badge off"}>
      {isOn ? "ON" : "OFF"}
    </span>
  </div>
));
