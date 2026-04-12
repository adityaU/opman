import React, { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { fetchThemes, switchTheme } from "./api";
import type { ThemePreview, ThemeColors } from "./api";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { applyThemeToCss } from "./utils/theme";
import { Palette, Search, Loader2, X, Layers, Square, Monitor, Sun, Moon } from "lucide-react";
import type { Appearance } from "./utils/appearance";
import {
  resolveAppearance, applyAppearanceClass, persistAppearance,
  storeThemePair, resolveThemeColors,
} from "./utils/appearance";

export type ThemeMode = "glassy" | "flat";

const THEME_MODE_KEY = "opman-theme-mode";

/** Read persisted theme mode from localStorage */
export function getPersistedThemeMode(): ThemeMode {
  try {
    const v = localStorage.getItem(THEME_MODE_KEY);
    if (v === "glassy") return "glassy";
  } catch { /* ignore */ }
  return "flat";
}

/** Persist theme mode to localStorage */
function persistThemeMode(mode: ThemeMode) {
  try { localStorage.setItem(THEME_MODE_KEY, mode); } catch { /* ignore */ }
}

/** Apply or remove the flat-theme class on <html> */
export function applyThemeMode(mode: ThemeMode) {
  const root = document.documentElement;
  if (mode === "flat") {
    root.classList.add("flat-theme");
  } else {
    root.classList.remove("flat-theme");
  }
}

/** Resolve ThemePreview colors based on appearance. */
function previewColors(theme: ThemePreview, a: Appearance): ThemeColors {
  return resolveAppearance(a) === "light" ? theme.light : theme.dark;
}

interface Props {
  onClose: () => void;
  onThemeApplied: (colors: ThemeColors) => void;
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
  appearance: Appearance;
  onAppearanceChange: (a: Appearance) => void;
}

export function ThemeSelectorModal({
  onClose, onThemeApplied, themeMode, onThemeModeChange,
  appearance, onAppearanceChange,
}: Props) {
  const [themes, setThemes] = useState<ThemePreview[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [applying, setApplying] = useState(false);
  const [localAppearance, setLocalAppearance] = useState<Appearance>(appearance);
  const inputRef = useRef<HTMLInputElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useFocusTrap(modalRef);

  // Save originals for revert on cancel
  const originalTheme = useRef<Record<string, string>>({});
  const originalMode = useRef<ThemeMode>(themeMode);
  const originalAppearance = useRef<Appearance>(appearance);

  useEffect(() => {
    const root = document.documentElement;
    const vars = [
      "--color-primary", "--color-secondary", "--color-accent",
      "--color-bg", "--color-bg-panel", "--color-bg-element",
      "--color-text", "--color-text-muted",
      "--color-border", "--color-border-active", "--color-border-subtle",
      "--color-error", "--color-warning", "--color-success", "--color-info",
    ];
    const saved: Record<string, string> = {};
    for (const v of vars) saved[v] = getComputedStyle(root).getPropertyValue(v).trim();
    originalTheme.current = saved;
    originalMode.current = themeMode;
    originalAppearance.current = appearance;

    fetchThemes()
      .then((data) => { setThemes(data); setLoading(false); })
      .catch(() => setLoading(false));

    inputRef.current?.focus();
  }, []);

  const filtered = useMemo(() => {
    if (!filter) return themes;
    const lower = filter.toLowerCase();
    return themes.filter((t) => t.name.toLowerCase().includes(lower));
  }, [themes, filter]);

  // Apply preview when selection or appearance changes
  useEffect(() => {
    if (filtered[selectedIdx]) {
      applyThemeToCss(previewColors(filtered[selectedIdx], localAppearance));
    }
  }, [selectedIdx, filtered, localAppearance]);

  useEffect(() => { setSelectedIdx(0); }, [filter]);

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
        if (filtered[selectedIdx]) handleApplyTheme(filtered[selectedIdx]);
      } else if (e.key === "Escape") {
        revertAndClose();
      }
    },
    [filtered, selectedIdx],
  );

  const handleHover = useCallback((theme: ThemePreview) => {
    applyThemeToCss(previewColors(theme, localAppearance));
  }, [localAppearance]);

  const revertAndClose = useCallback(() => {
    const root = document.documentElement;
    for (const [k, v] of Object.entries(originalTheme.current)) root.style.setProperty(k, v);
    // Revert mode
    if (themeMode !== originalMode.current) {
      onThemeModeChange(originalMode.current);
      applyThemeMode(originalMode.current);
      persistThemeMode(originalMode.current);
    }
    // Revert appearance
    if (localAppearance !== originalAppearance.current) {
      onAppearanceChange(originalAppearance.current);
      applyAppearanceClass(originalAppearance.current);
      persistAppearance(originalAppearance.current);
    }
    onClose();
  }, [onClose, themeMode, onThemeModeChange, localAppearance, onAppearanceChange]);

  const handleApplyTheme = useCallback(
    async (theme: ThemePreview) => {
      setApplying(true);
      try {
        const pair = await switchTheme(theme.name);
        const colors = resolveThemeColors(pair, localAppearance);
        storeThemePair(pair, localAppearance);
        applyThemeToCss(colors);
        onThemeApplied(colors);
        onAppearanceChange(localAppearance);
        persistAppearance(localAppearance);
        applyAppearanceClass(localAppearance);
        onClose();
      } catch {
        const colors = previewColors(theme, localAppearance);
        applyThemeToCss(colors);
        onThemeApplied(colors);
        onAppearanceChange(localAppearance);
        persistAppearance(localAppearance);
        applyAppearanceClass(localAppearance);
        onClose();
      } finally {
        setApplying(false);
      }
    },
    [onClose, onThemeApplied, localAppearance, onAppearanceChange],
  );

  const handleModeSwitch = useCallback(
    (mode: ThemeMode) => { onThemeModeChange(mode); applyThemeMode(mode); persistThemeMode(mode); },
    [onThemeModeChange],
  );

  const handleAppearanceSwitch = useCallback((a: Appearance) => {
    setLocalAppearance(a);
    applyAppearanceClass(a);
    // Preview is updated via the useEffect above
  }, []);

  return (
    <div className="modal-backdrop" onClick={revertAndClose}>
      <div
        className="theme-selector"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        role="dialog" aria-modal="true" aria-label="Appearance settings" ref={modalRef}
      >
        {/* Header */}
        <div className="theme-selector-header">
          <Palette size={14} />
          <span>Appearance</span>
          <button className="theme-selector-close" onClick={revertAndClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        {/* Appearance switcher (System / Light / Dark) */}
        <div className="theme-appearance-switcher">
          {([["system", Monitor, "System", "Follow OS"] as const,
            ["light", Sun, "Light", "Light colors"] as const,
            ["dark", Moon, "Dark", "Dark colors"] as const,
          ]).map(([key, Icon, label, desc]) => (
            <button
              key={key}
              className={`theme-mode-option${localAppearance === key ? " active" : ""}`}
              onClick={() => handleAppearanceSwitch(key)}
            >
              <Icon size={16} />
              <div className="theme-mode-text">
                <span className="theme-mode-label">{label}</span>
                <span className="theme-mode-desc">{desc}</span>
              </div>
            </button>
          ))}
        </div>

        {/* Mode switcher (Glassy / Flat) */}
        <div className="theme-mode-switcher">
          <button className={`theme-mode-option${themeMode === "glassy" ? " active" : ""}`} onClick={() => handleModeSwitch("glassy")}>
            <Layers size={16} />
            <div className="theme-mode-text">
              <span className="theme-mode-label">Glassy</span>
              <span className="theme-mode-desc">Translucent blur</span>
            </div>
          </button>
          <button className={`theme-mode-option${themeMode === "flat" ? " active" : ""}`} onClick={() => handleModeSwitch("flat")}>
            <Square size={16} />
            <div className="theme-mode-text">
              <span className="theme-mode-label">Flat</span>
              <span className="theme-mode-desc">Solid surfaces</span>
            </div>
          </button>
        </div>

        {/* Color Themes label */}
        <div className="theme-section-label">
          <span>Color Themes</span>
          <span className="theme-selector-count">{filtered.length}</span>
        </div>

        {/* Search */}
        <div className="theme-selector-search">
          <Search size={13} />
          <input ref={inputRef} className="theme-selector-input" type="text"
            placeholder="Search themes..." value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>

        {/* Theme grid */}
        <div className="theme-selector-grid">
          {loading ? (
            <div className="theme-selector-loading"><Loader2 size={16} className="spinning" /><span>Loading themes...</span></div>
          ) : filtered.length === 0 ? (
            <div className="theme-selector-empty">No themes found</div>
          ) : (
            filtered.map((theme, idx) => {
              const c = previewColors(theme, localAppearance);
              return (
                <button key={theme.name}
                  className={`theme-card${idx === selectedIdx ? " selected" : ""}`}
                  onClick={() => handleApplyTheme(theme)}
                  onMouseEnter={() => { handleHover(theme); setSelectedIdx(idx); }}
                >
                  <div className="theme-card-preview">
                    <span style={{ background: c.background, flex: 2 }} />
                    <span style={{ background: c.primary, flex: 1 }} />
                    <span style={{ background: c.secondary, flex: 1 }} />
                    <span style={{ background: c.accent, flex: 1 }} />
                    <span style={{ background: c.text, flex: 1 }} />
                  </div>
                  <span className="theme-card-name">{theme.name}</span>
                  {applying && idx === selectedIdx && <Loader2 size={12} className="spinning" />}
                </button>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="theme-selector-footer">
          <kbd>Up/Down</kbd> Navigate <kbd>Enter</kbd> Apply <kbd>Esc</kbd> Cancel
        </div>
      </div>
    </div>
  );
}
