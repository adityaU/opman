import React, { useState, useEffect, useMemo, useRef } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Search } from "lucide-react";
import { CommandPaletteProps } from "./types";
import { buildPaletteItems } from "./items";
import { filterItems, groupItems } from "./helpers";

export function CommandPalette(props: CommandPaletteProps) {
  const { onClose, sessionId } = props;
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const paletteRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(paletteRef);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Memoize on the values that actually change the item list, not the
  // whole props object (which gets a new reference every render).
  const items = useMemo(
    () => buildPaletteItems(props),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [sessionId],
  );
  const filtered = useMemo(() => filterItems(items, query), [items, query]);
  const grouped = useMemo(() => groupItems(filtered), [filtered]);

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
      <div
        ref={paletteRef}
        className="command-palette"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
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
                <div className="command-palette-section-title">
                  {group.category}
                </div>
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
