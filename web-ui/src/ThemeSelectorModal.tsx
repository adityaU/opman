import React, { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { fetchThemes, switchTheme } from "./api";
import type { ThemePreview, ThemeColors } from "./api";
import { Palette, Search, Check, Loader2, X } from "lucide-react";

interface Props {
  onClose: () => void;
  onThemeApplied: (colors: ThemeColors) => void;
}

/** Apply theme colors as CSS custom properties on :root for live preview */
function previewTheme(colors: ThemeColors) {
  const root = document.documentElement;
  root.style.setProperty("--color-primary", colors.primary);
  root.style.setProperty("--color-secondary", colors.secondary);
  root.style.setProperty("--color-accent", colors.accent);
  root.style.setProperty("--color-bg", colors.background);
  root.style.setProperty("--color-bg-panel", colors.background_panel);
  root.style.setProperty("--color-bg-element", colors.background_element);
  root.style.setProperty("--color-text", colors.text);
  root.style.setProperty("--color-text-muted", colors.text_muted);
  root.style.setProperty("--color-border", colors.border);
  root.style.setProperty("--color-border-active", colors.border_active);
  root.style.setProperty("--color-border-subtle", colors.border_subtle);
  root.style.setProperty("--color-error", colors.error);
  root.style.setProperty("--color-warning", colors.warning);
  root.style.setProperty("--color-success", colors.success);
  root.style.setProperty("--color-info", colors.info);
}

export function ThemeSelectorModal({ onClose, onThemeApplied }: Props) {
  const [themes, setThemes] = useState<ThemePreview[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [applying, setApplying] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Save the original theme so we can revert on cancel
  const originalTheme = useRef<Record<string, string>>({});

  useEffect(() => {
    // Capture current CSS vars
    const root = document.documentElement;
    const vars = [
      "--color-primary", "--color-secondary", "--color-accent",
      "--color-bg", "--color-bg-panel", "--color-bg-element",
      "--color-text", "--color-text-muted",
      "--color-border", "--color-border-active", "--color-border-subtle",
      "--color-error", "--color-warning", "--color-success", "--color-info",
    ];
    const saved: Record<string, string> = {};
    for (const v of vars) {
      saved[v] = getComputedStyle(root).getPropertyValue(v).trim();
    }
    originalTheme.current = saved;

    // Fetch themes
    fetchThemes()
      .then((data) => {
        setThemes(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));

    inputRef.current?.focus();
  }, []);

  const filtered = useMemo(() => {
    if (!filter) return themes;
    const lower = filter.toLowerCase();
    return themes.filter((t) => t.name.toLowerCase().includes(lower));
  }, [themes, filter]);

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (filtered[selectedIdx]) {
          applyTheme(filtered[selectedIdx]);
        }
      } else if (e.key === "Escape") {
        revertAndClose();
      }
    },
    [filtered, selectedIdx]
  );

  // Reset selected index when filter changes
  useEffect(() => {
    setSelectedIdx(0);
  }, [filter]);

  // Live preview on hover / selection change
  const handleHover = useCallback((theme: ThemePreview) => {
    previewTheme(theme.colors);
  }, []);

  // Live preview on arrow key selection
  useEffect(() => {
    if (filtered[selectedIdx]) {
      previewTheme(filtered[selectedIdx].colors);
    }
  }, [selectedIdx, filtered]);

  const revertAndClose = useCallback(() => {
    // Revert to original theme
    const root = document.documentElement;
    for (const [k, v] of Object.entries(originalTheme.current)) {
      root.style.setProperty(k, v);
    }
    onClose();
  }, [onClose]);

  const applyTheme = useCallback(
    async (theme: ThemePreview) => {
      setApplying(true);
      try {
        const colors = await switchTheme(theme.name);
        previewTheme(colors);
        onThemeApplied(colors);
        onClose();
      } catch {
        // If switch fails, still apply the preview colors locally
        previewTheme(theme.colors);
        onThemeApplied(theme.colors);
        onClose();
      } finally {
        setApplying(false);
      }
    },
    [onClose, onThemeApplied]
  );

  return (
    <div className="modal-backdrop" onClick={revertAndClose}>
      <div
        className="theme-selector"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <div className="theme-selector-header">
          <Palette size={14} />
          <span>Select Theme</span>
          <span className="theme-selector-count">
            {filtered.length} theme{filtered.length !== 1 ? "s" : ""}
          </span>
          <button className="theme-selector-close" onClick={revertAndClose}>
            <X size={14} />
          </button>
        </div>

        {/* Search */}
        <div className="theme-selector-search">
          <Search size={13} />
          <input
            ref={inputRef}
            className="theme-selector-input"
            type="text"
            placeholder="Search themes..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>

        {/* Theme grid */}
        <div className="theme-selector-grid">
          {loading ? (
            <div className="theme-selector-loading">
              <Loader2 size={16} className="spinning" />
              <span>Loading themes...</span>
            </div>
          ) : filtered.length === 0 ? (
            <div className="theme-selector-empty">No themes found</div>
          ) : (
            filtered.map((theme, idx) => (
              <button
                key={theme.name}
                className={`theme-card ${idx === selectedIdx ? "selected" : ""}`}
                onClick={() => applyTheme(theme)}
                onMouseEnter={() => {
                  handleHover(theme);
                  setSelectedIdx(idx);
                }}
              >
                {/* Color swatches */}
                <div className="theme-card-swatches">
                  <span
                    className="theme-swatch"
                    style={{ background: theme.colors.background }}
                  />
                  <span
                    className="theme-swatch"
                    style={{ background: theme.colors.primary }}
                  />
                  <span
                    className="theme-swatch"
                    style={{ background: theme.colors.secondary }}
                  />
                  <span
                    className="theme-swatch"
                    style={{ background: theme.colors.accent }}
                  />
                  <span
                    className="theme-swatch"
                    style={{ background: theme.colors.success }}
                  />
                  <span
                    className="theme-swatch"
                    style={{ background: theme.colors.error }}
                  />
                </div>
                <span className="theme-card-name">{theme.name}</span>
                {applying && idx === selectedIdx && (
                  <Loader2 size={12} className="spinning" />
                )}
              </button>
            ))
          )}
        </div>

        {/* Footer hint */}
        <div className="theme-selector-footer">
          <kbd>Up/Down</kbd> Navigate
          <kbd>Enter</kbd> Apply
          <kbd>Esc</kbd> Cancel
        </div>
      </div>
    </div>
  );
}
