import React, { useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Keyboard, X } from "lucide-react";

interface Props {
  onClose: () => void;
}

interface ShortcutEntry {
  key: string;
  description: string;
}

interface ShortcutSection {
  title: string;
  entries: ShortcutEntry[];
}

/** All keyboard shortcuts available in the web UI, organized by category */
const SECTIONS: ShortcutSection[] = [
  {
    title: "General",
    entries: [
      { key: "Cmd+Shift+P", description: "Command Palette" },
      { key: "Cmd+Shift+N", description: "New Session" },
      { key: "Cmd+B", description: "Toggle Sidebar" },
      { key: "Cmd+`", description: "Toggle Terminal" },
      { key: "Cmd+Shift+E", description: "Toggle Editor" },
      { key: "Cmd+Shift+G", description: "Toggle Git" },
      { key: "?", description: "Keybinding Cheatsheet" },
    ],
  },
  {
    title: "Chat",
    entries: [
      { key: "Enter", description: "Send message" },
      { key: "Shift+Enter", description: "New line" },
      { key: "/", description: "Slash commands" },
      { key: "Cmd+'", description: "Model Picker" },
    ],
  },
  {
    title: "Modals",
    entries: [
      { key: "Cmd+Shift+T", description: "Todo Panel" },
      { key: "Cmd+Shift+S", description: "Session Selector" },
      { key: "Cmd+Shift+K", description: "Context Input" },
      { key: "Cmd+,", description: "Settings" },
      { key: "Esc", description: "Close modal/dialog" },
    ],
  },
  {
    title: "Slash Commands",
    entries: [
      { key: "/new", description: "New session" },
      { key: "/model", description: "Switch model" },
      { key: "/theme", description: "Switch theme" },
      { key: "/compact", description: "Compact history" },
      { key: "/undo", description: "Undo last action" },
      { key: "/redo", description: "Redo last action" },
      { key: "/fork", description: "Fork session" },
      { key: "/share", description: "Share session" },
      { key: "/agent", description: "Switch agent" },
      { key: "/terminal", description: "Toggle terminal" },
    ],
  },
  {
    title: "Navigation",
    entries: [
      { key: "Up/Down", description: "Navigate lists" },
      { key: "Tab", description: "Switch tabs / next field" },
      { key: "Enter", description: "Select / confirm" },
    ],
  },
];

export function CheatsheetModal({ onClose }: Props) {
  const modalRef = useRef<HTMLDivElement>(null);
  useEscape(onClose);
  useFocusTrap(modalRef);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="cheatsheet-modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts" ref={modalRef}>
        <div className="cheatsheet-header">
          <Keyboard size={14} />
          <span>Keybindings</span>
          <button className="cheatsheet-close" onClick={onClose} aria-label="Close keybindings">
            <X size={14} />
          </button>
        </div>

        <div className="cheatsheet-body">
          {SECTIONS.map((section) => (
            <div key={section.title} className="cheatsheet-section">
              <div className="cheatsheet-section-title">{section.title}</div>
              {section.entries.map((entry) => (
                <div key={entry.key} className="cheatsheet-row">
                  <kbd className="cheatsheet-key">{entry.key}</kbd>
                  <span className="cheatsheet-desc">{entry.description}</span>
                </div>
              ))}
            </div>
          ))}
        </div>

        <div className="cheatsheet-footer">
          <kbd>Esc</kbd> Close
          <kbd>?</kbd> Toggle
        </div>
      </div>
    </div>
  );
}
