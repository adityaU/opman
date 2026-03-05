import React, { useState, useEffect, useMemo, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { Search, Command } from "lucide-react";

interface Props {
  onClose: () => void;
  onCommand: (command: string, args?: string) => void;
  onNewSession: () => void;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onOpenModelPicker: () => void;
  sessionId: string | null;
}

interface PaletteItem {
  id: string;
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
  sessionId,
}: Props) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEscape(onClose);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const items: PaletteItem[] = useMemo(
    () => [
      {
        id: "new-session",
        label: "New Session",
        shortcut: "Cmd+Shift+N",
        handler: () => {
          onClose();
          onNewSession();
        },
      },
      {
        id: "model-picker",
        label: "Choose Model",
        shortcut: "Cmd+'",
        handler: () => {
          onOpenModelPicker();
        },
      },
      {
        id: "toggle-sidebar",
        label: "Toggle Sidebar",
        shortcut: "Cmd+B",
        handler: () => {
          onClose();
          onToggleSidebar();
        },
      },
      {
        id: "toggle-terminal",
        label: "Toggle Terminal",
        shortcut: "Cmd+`",
        handler: () => {
          onClose();
          onToggleTerminal();
        },
      },
      ...(sessionId
        ? [
            {
              id: "compact",
              label: "Compact History",
              description: "Compact conversation to reduce tokens",
              handler: () => {
                onClose();
                onCommand("compact");
              },
            },
            {
              id: "undo",
              label: "Undo",
              description: "Undo last action",
              handler: () => {
                onClose();
                onCommand("undo");
              },
            },
            {
              id: "redo",
              label: "Redo",
              description: "Redo last action",
              handler: () => {
                onClose();
                onCommand("redo");
              },
            },
            {
              id: "fork",
              label: "Fork Session",
              description: "Create a copy of this session",
              handler: () => {
                onClose();
                onCommand("fork");
              },
            },
            {
              id: "share",
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
      <div className="command-palette" onClick={(e) => e.stopPropagation()}>
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
            filtered.map((item, idx) => (
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
            ))
          )}
        </div>
      </div>
    </div>
  );
}
