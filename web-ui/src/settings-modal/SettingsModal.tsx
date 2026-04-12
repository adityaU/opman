import React, { useState, useCallback, useRef, useEffect } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Settings, X } from "lucide-react";
import { buildSettingsItems } from "./items";

interface Props {
  onClose: () => void;
  onOpenThemeSelector: () => void;
  onOpenCheatsheet: () => void;
  onOpenNotificationPrefs?: () => void;
  onOpenAssistantCenter?: () => void;
  onOpenMemory?: () => void;
  onOpenAutonomy?: () => void;
  onOpenRoutines?: () => void;
  onOpenDelegation?: () => void;
  onOpenWorkspaceManager?: () => void;
  onOpenInbox?: () => void;
  onOpenMissions?: () => void;
  sidebarOpen: boolean;
  terminalOpen: boolean;
  neovimOpen: boolean;
  gitOpen: boolean;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onToggleNeovim: () => void;
  onToggleGit: () => void;
}

export function SettingsModal(props: Props) {
  const { onClose } = props;
  const [selectedIdx, setSelectedIdx] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const items = buildSettingsItems(props);

  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIdx] as HTMLElement;
    if (item) item.scrollIntoView({ block: "nearest" });
  }, [selectedIdx]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, items.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        items[selectedIdx]?.handler();
      }
    },
    [items, selectedIdx]
  );

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="settings-modal"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={0}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        ref={modalRef}
      >
        <div className="settings-header">
          <Settings size={14} />
          <span>Settings</span>
          <button className="settings-close" onClick={onClose} aria-label="Close settings">
            <X size={14} />
          </button>
        </div>

        <div className="settings-list" ref={listRef}>
          {items.map((item, idx) => (
            <button
              key={item.id}
              className={`settings-item ${idx === selectedIdx ? "selected" : ""}`}
              onClick={item.handler}
              onMouseEnter={() => setSelectedIdx(idx)}
            >
              <div className="settings-item-left">
                <span className="settings-item-icon">{item.icon}</span>
                <div className="settings-item-text">
                  <span className="settings-item-label">{item.label}</span>
                  <span className="settings-item-desc">{item.description}</span>
                </div>
              </div>
              {item.type === "toggle" && (
                <span className={`settings-toggle ${item.value ? "on" : "off"}`}>
                  {item.value ? "ON" : "OFF"}
                </span>
              )}
              {item.type === "action" && (
                <span className="settings-action-arrow">&rsaquo;</span>
              )}
            </button>
          ))}
        </div>

        <div className="settings-footer">
          <kbd>Up/Down</kbd> Navigate
          <kbd>Enter</kbd> Toggle / Open
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
