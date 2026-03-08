import React, { useState, useEffect, useMemo, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Search, Command } from "lucide-react";

interface Props {
  onClose: () => void;
  onCommand: (command: string, args?: string) => void;
  onNewSession: () => void;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onOpenModelPicker: () => void;
  onOpenCheatsheet: () => void;
  onOpenTodoPanel: () => void;
  onOpenSessionSelector: () => void;
  onOpenContextInput: () => void;
  onOpenSettings: () => void;
  onOpenWatcher: () => void;
  onOpenContextWindow: () => void;
  onOpenDiffReview: () => void;
  onOpenSearch: () => void;
  onOpenCrossSearch: () => void;
  onOpenSplitView?: () => void;
  onOpenSessionGraph?: () => void;
  onOpenSessionDashboard?: () => void;
  onOpenActivityFeed?: () => void;
  onOpenNotificationPrefs?: () => void;
  onOpenInbox?: () => void;
  onOpenAssistantCenter?: () => void;
  onOpenMemory?: () => void;
  onOpenAutonomy?: () => void;
  onOpenRoutines?: () => void;
  onOpenDelegation?: () => void;
  onOpenWorkspaceManager?: () => void;
  onOpenMissions?: () => void;
  sessionId: string | null;
}

interface PaletteItem {
  id: string;
  category: string;
  label: string;
  description?: string;
  shortcut?: string;
  handler: () => void;
}

export function CommandPalette({
  onClose,
  onCommand,
  onNewSession,
  onToggleSidebar,
  onToggleTerminal,
  onOpenModelPicker,
  onOpenCheatsheet,
  onOpenTodoPanel,
  onOpenSessionSelector,
  onOpenContextInput,
  onOpenSettings,
  onOpenWatcher,
  onOpenContextWindow,
  onOpenDiffReview,
  onOpenSearch,
  onOpenCrossSearch,
  onOpenSplitView,
  onOpenSessionGraph,
  onOpenSessionDashboard,
  onOpenActivityFeed,
  onOpenNotificationPrefs,
  onOpenInbox,
  onOpenAssistantCenter,
  onOpenMemory,
  onOpenAutonomy,
  onOpenRoutines,
  onOpenDelegation,
  onOpenWorkspaceManager,
  onOpenMissions,
  sessionId,
}: Props) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const paletteRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(paletteRef);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const items: PaletteItem[] = useMemo(
    () => [
      {
        id: "new-session",
        category: "Sessions",
        label: "New Session",
        shortcut: "Cmd+Shift+N",
        handler: () => {
          onClose();
          onNewSession();
        },
      },
      {
        id: "model-picker",
        category: "Core",
        label: "Choose Model",
        shortcut: "Cmd+'",
        handler: () => {
          onOpenModelPicker();
        },
      },
      {
        id: "toggle-sidebar",
        category: "Layout",
        label: "Toggle Sidebar",
        shortcut: "Cmd+B",
        handler: () => {
          onClose();
          onToggleSidebar();
        },
      },
      {
        id: "toggle-terminal",
        category: "Layout",
        label: "Toggle Terminal",
        shortcut: "Cmd+`",
        handler: () => {
          onClose();
          onToggleTerminal();
        },
      },
      {
        id: "cheatsheet",
        category: "Core",
        label: "Keyboard Shortcuts",
        shortcut: "?",
        handler: () => {
          onClose();
          onOpenCheatsheet();
        },
      },
      {
        id: "session-selector",
        category: "Sessions",
        label: "Select Session",
        description: "Search across all projects",
        shortcut: "Cmd+Shift+S",
        handler: () => {
          onClose();
          onOpenSessionSelector();
        },
      },
      {
        id: "settings",
        category: "Core",
        label: "Settings",
        description: "Configure panels and theme",
        shortcut: "Cmd+,",
        handler: () => {
          onClose();
          onOpenSettings();
        },
      },
      {
        id: "watcher",
        category: "Sessions",
        label: "Session Watcher",
        description: "Monitor and auto-continue sessions",
        shortcut: "Cmd+Shift+W",
        handler: () => {
          onClose();
          onOpenWatcher();
        },
      },
      {
        id: "context-window",
        category: "Analysis",
        label: "Context Window",
        description: "View token usage breakdown",
        shortcut: "Cmd+Shift+C",
        handler: () => {
          onClose();
          onOpenContextWindow();
        },
      },
      {
        id: "diff-review",
        category: "Analysis",
        label: "Diff Review",
        description: "Review file changes made by AI",
        shortcut: "Cmd+Shift+D",
        handler: () => {
          onClose();
          onOpenDiffReview();
        },
      },
      {
        id: "search",
        category: "Search",
        label: "Search in Conversation",
        description: "Find text in the current session",
        shortcut: "Cmd+F",
        handler: () => {
          onClose();
          onOpenSearch();
        },
      },
      {
        id: "cross-search",
        category: "Search",
        label: "Search All Sessions",
        description: "Search across all sessions in project",
        shortcut: "Cmd+Shift+F",
        handler: () => {
          onClose();
          onOpenCrossSearch();
        },
      },
      {
        id: "split-view",
        category: "Layout",
        label: "Split View",
        description: "View two sessions side by side",
        shortcut: "⌘\\",
        handler: () => {
          onClose();
          onOpenSplitView?.();
        },
      },
      {
        id: "session-graph",
        category: "Sessions",
        label: "Session Graph",
        description: "View session dependency tree",
        shortcut: "⌘⇧H",
        handler: () => {
          onClose();
          onOpenSessionGraph?.();
        },
      },
      {
        id: "session-dashboard",
        category: "Sessions",
        label: "Session Dashboard",
        description: "Overview of all running sessions",
        shortcut: "⌘⇧O",
        handler: () => {
          onClose();
          onOpenSessionDashboard?.();
        },
      },
      {
        id: "activity-feed",
        category: "Sessions",
        label: "Activity Feed",
        description: "View real-time session activity",
        shortcut: "⌘⇧A",
        handler: () => {
          onClose();
          onOpenActivityFeed?.();
        },
      },
      {
        id: "notification-prefs",
        category: "Assistant",
        label: "Notification Preferences",
        description: "Configure session alerts",
        shortcut: "⌘⇧I",
        handler: () => {
          onClose();
          onOpenNotificationPrefs?.();
        },
      },
      {
        id: "assistant-center",
        category: "Assistant",
        label: "Assistant Center",
        description: "Open the assistant cockpit",
        shortcut: "⌘⇧.",
        handler: () => {
          onClose();
          onOpenAssistantCenter?.();
        },
      },
      {
        id: "assistant-inbox",
        category: "Assistant",
        label: "Assistant Inbox",
        description: "Review everything that needs attention",
        shortcut: "⌘⇧U",
        handler: () => {
          onClose();
          onOpenInbox?.();
        },
      },
      {
        id: "missions",
        category: "Assistant",
        label: "Missions",
        description: "Track high-level goals above sessions",
        shortcut: "⌘⇧M",
        handler: () => {
          onClose();
          onOpenMissions?.();
        },
      },
      {
        id: "delegation-board",
        category: "Assistant",
        label: "Delegation Board",
        description: "Track delegated work and linked outputs",
        shortcut: "⌘⇧B",
        handler: () => {
          onClose();
          onOpenDelegation?.();
        },
      },
      {
        id: "routines",
        category: "Assistant",
        label: "Routines",
        description: "Manage recurring assistant workflows",
        shortcut: "⌘⇧R",
        handler: () => {
          onClose();
          onOpenRoutines?.();
        },
      },
      {
        id: "autonomy",
        category: "Assistant",
        label: "Autonomy",
        description: "Choose proactive assistant mode",
        shortcut: "⌘⇧J",
        handler: () => {
          onClose();
          onOpenAutonomy?.();
        },
      },
      {
        id: "personal-memory",
        category: "Assistant",
        label: "Personal Memory",
        description: "Store preferences and working norms",
        shortcut: "⌘⇧Y",
        handler: () => {
          onClose();
          onOpenMemory?.();
        },
      },
      {
        id: "workspace-manager",
        category: "Assistant",
        label: "Workspaces",
        description: "Save and restore workspace layouts",
        shortcut: "⌘⇧L",
        handler: () => {
          onClose();
          onOpenWorkspaceManager?.();
        },
      },
      ...(sessionId
        ? [
            {
              id: "todo-panel",
              category: "Sessions",
              label: "Todo Panel",
              description: "View session todos",
              shortcut: "Cmd+Shift+T",
              handler: () => {
                onClose();
                onOpenTodoPanel();
              },
            },
            {
              id: "context-input",
              category: "Sessions",
              label: "Send Context",
              description: "Send context to the AI session",
              shortcut: "Cmd+Shift+K",
              handler: () => {
                onClose();
                onOpenContextInput();
              },
            },
            {
              id: "compact",
              category: "Sessions",
              label: "Compact History",
              description: "Compact conversation to reduce tokens",
              handler: () => {
                onClose();
                onCommand("compact");
              },
            },
            {
              id: "undo",
              category: "Sessions",
              label: "Undo",
              description: "Undo last action",
              handler: () => {
                onClose();
                onCommand("undo");
              },
            },
            {
              id: "redo",
              category: "Sessions",
              label: "Redo",
              description: "Redo last action",
              handler: () => {
                onClose();
                onCommand("redo");
              },
            },
            {
              id: "fork",
              category: "Sessions",
              label: "Fork Session",
              description: "Create a copy of this session",
              handler: () => {
                onClose();
                onCommand("fork");
              },
            },
            {
              id: "share",
              category: "Sessions",
              label: "Share Session",
              description: "Get a shareable link",
              handler: () => {
                onClose();
                onCommand("share");
              },
            },
          ]
        : []),
    ],
    [
      onClose,
      onCommand,
      onNewSession,
      onToggleSidebar,
      onToggleTerminal,
      onOpenModelPicker,
      onOpenCheatsheet,
      onOpenTodoPanel,
      onOpenSessionSelector,
      onOpenContextInput,
      onOpenSettings,
      onOpenWatcher,
      onOpenContextWindow,
      onOpenDiffReview,
      onOpenSearch,
      onOpenCrossSearch,
      onOpenSplitView,
      onOpenSessionGraph,
      onOpenSessionDashboard,
      onOpenActivityFeed,
      onOpenNotificationPrefs,
      onOpenInbox,
      onOpenAssistantCenter,
      onOpenMemory,
      onOpenAutonomy,
      onOpenRoutines,
      onOpenDelegation,
      onOpenWorkspaceManager,
      onOpenMissions,
      sessionId,
    ]
  );

  const filtered = useMemo(() => {
    if (!query) return items;
    const lq = query.toLowerCase();
    return items.filter(
      (i) =>
        i.label.toLowerCase().includes(lq) ||
        i.description?.toLowerCase().includes(lq)
    );
  }, [items, query]);

  const grouped = useMemo(() => {
    const sections: Array<{ category: string; items: PaletteItem[] }> = [];
    for (const item of filtered) {
      const current = sections[sections.length - 1];
      if (current && current.category === item.category) {
        current.items.push(item);
      } else {
        sections.push({ category: item.category, items: [item] });
      }
    }
    return sections;
  }, [filtered]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filtered[selectedIndex]) {
        filtered[selectedIndex].handler();
      }
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div ref={paletteRef} className="command-palette" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="command-palette-input-row">
          <Search size={16} className="command-palette-icon" />
          <input
            ref={inputRef}
            className="command-palette-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a command..."
          />
        </div>
        <div className="command-palette-results">
          {filtered.length === 0 ? (
            <div className="command-palette-empty">No commands found</div>
          ) : (
            grouped.map((group) => (
              <div key={group.category} className="command-palette-section">
                <div className="command-palette-section-title">{group.category}</div>
                {group.items.map((item) => {
                  const idx = filtered.findIndex((entry) => entry.id === item.id);
                  return (
                    <button
                      key={item.id}
                      className={`command-palette-item ${idx === selectedIndex ? "selected" : ""}`}
                      onClick={item.handler}
                      onMouseEnter={() => setSelectedIndex(idx)}
                    >
                      <div className="command-palette-item-left">
                        <span className="command-palette-label">{item.label}</span>
                        {item.description && (
                          <span className="command-palette-desc">
                            {item.description}
                          </span>
                        )}
                      </div>
                      {item.shortcut && (
                        <kbd className="command-palette-shortcut">
                          {item.shortcut}
                        </kbd>
                      )}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
