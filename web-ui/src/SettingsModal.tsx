import React, { useState, useCallback, useRef, useEffect } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Settings, X, Palette, Monitor, Keyboard, Bell, Layers, Brain, Bot, Clock3, Inbox, Target } from "lucide-react";

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

interface SettingItem {
  id: string;
  label: string;
  description: string;
  type: "toggle" | "action";
  value?: boolean;
  icon: React.ReactNode;
  handler: () => void;
}

export function SettingsModal({
  onClose,
  onOpenThemeSelector,
  onOpenCheatsheet,
  onOpenNotificationPrefs,
  onOpenAssistantCenter,
  onOpenMemory,
  onOpenAutonomy,
  onOpenRoutines,
  onOpenDelegation,
  onOpenWorkspaceManager,
  onOpenInbox,
  onOpenMissions,
  sidebarOpen,
  terminalOpen,
  neovimOpen,
  gitOpen,
  onToggleSidebar,
  onToggleTerminal,
  onToggleNeovim,
  onToggleGit,
}: Props) {
  const [selectedIdx, setSelectedIdx] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const items: SettingItem[] = [
    {
      id: "theme",
      label: "Theme",
      description: "Choose a color theme",
      type: "action",
      icon: <Palette size={14} />,
      handler: () => {
        onClose();
        onOpenThemeSelector();
      },
    },
    {
      id: "keybindings",
      label: "Keybindings",
      description: "View keyboard shortcuts",
      type: "action",
      icon: <Keyboard size={14} />,
      handler: () => {
        onClose();
        onOpenCheatsheet();
      },
    },
    {
      id: "notifications",
      label: "Notifications",
      description: "Configure session alerts",
      type: "action",
      icon: <Bell size={14} />,
      handler: () => {
        onClose();
        onOpenNotificationPrefs?.();
      },
    },
    {
      id: "assistant-center",
      label: "Assistant Center",
      description: "Open the assistant cockpit",
      type: "action",
      icon: <Bot size={14} />,
      handler: () => {
        onClose();
        onOpenAssistantCenter?.();
      },
    },
    {
      id: "inbox",
      label: "Inbox",
      description: "Review items that need your attention",
      type: "action",
      icon: <Inbox size={14} />,
      handler: () => {
        onClose();
        onOpenInbox?.();
      },
    },
    {
      id: "missions",
      label: "Missions",
      description: "Track high-level goals above sessions",
      type: "action",
      icon: <Target size={14} />,
      handler: () => {
        onClose();
        onOpenMissions?.();
      },
    },
    {
      id: "memory",
      label: "Personal Memory",
      description: "Store stable preferences and constraints",
      type: "action",
      icon: <Brain size={14} />,
      handler: () => {
        onClose();
        onOpenMemory?.();
      },
    },
    {
      id: "autonomy",
      label: "Autonomy",
      description: "Choose how proactive opman may be",
      type: "action",
      icon: <Bot size={14} />,
      handler: () => {
        onClose();
        onOpenAutonomy?.();
      },
    },
    {
      id: "routines",
      label: "Routines",
      description: "Manage scheduled and triggered routines",
      type: "action",
      icon: <Clock3 size={14} />,
      handler: () => {
        onClose();
        onOpenRoutines?.();
      },
    },
    {
      id: "delegation",
      label: "Delegation Board",
      description: "Track delegated work and linked outputs",
      type: "action",
      icon: <Layers size={14} />,
      handler: () => {
        onClose();
        onOpenDelegation?.();
      },
    },
    {
      id: "workspaces",
      label: "Workspaces",
      description: "Save and restore workspace layouts",
      type: "action",
      icon: <Layers size={14} />,
      handler: () => {
        onClose();
        onOpenWorkspaceManager?.();
      },
    },
    {
      id: "sidebar",
      label: "Sidebar",
      description: "Show/hide the sidebar panel",
      type: "toggle",
      value: sidebarOpen,
      icon: <Monitor size={14} />,
      handler: onToggleSidebar,
    },
    {
      id: "terminal",
      label: "Terminal",
      description: "Show/hide the terminal panel",
      type: "toggle",
      value: terminalOpen,
      icon: <Monitor size={14} />,
      handler: onToggleTerminal,
    },
    {
      id: "neovim",
      label: "Editor",
      description: "Show/hide the code editor panel",
      type: "toggle",
      value: neovimOpen,
      icon: <Monitor size={14} />,
      handler: onToggleNeovim,
    },
    {
      id: "git",
      label: "Git Panel",
      description: "Show/hide the git panel",
      type: "toggle",
      value: gitOpen,
      icon: <Monitor size={14} />,
      handler: onToggleGit,
    },
  ];

  // Scroll selected into view
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
        {/* Header */}
        <div className="settings-header">
          <Settings size={14} />
          <span>Settings</span>
          <button className="settings-close" onClick={onClose} aria-label="Close settings">
            <X size={14} />
          </button>
        </div>

        {/* Settings list */}
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

        {/* Footer */}
        <div className="settings-footer">
          <kbd>Up/Down</kbd> Navigate
          <kbd>Enter</kbd> Toggle / Open
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
