import React, { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { fetchThemes, switchTheme } from "../api";
import type { ThemePreview, ThemeColors } from "../api";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { applyThemeToCss } from "../utils/theme";
import { Palette, Search, X } from "lucide-react";
import type { Appearance } from "../utils/appearance";
import {
  resolveAppearance, applyAppearanceClass, persistAppearance,
  storeThemePair, resolveThemeColors, activeThemeName,
} from "../utils/appearance";
import { applyThemeMode, persistThemeMode, type ThemeMode } from "./persistence";
import { AppearanceSwitcher, ModeSwitcher, ThemeGrid } from "./controls";

/** Resolve ThemePreview colors based on appearance. */
function previewColors(theme: ThemePreview, a: Appearance): ThemeColors {
  return resolveAppearance(a) === "light" ? theme.light : theme.dark;
}

/** CSS variables the modal restores when the user cancels. */
const PREVIEW_VARS = [
  "--color-primary", "--color-secondary", "--color-accent",
  "--color-bg", "--color-bg-panel", "--color-bg-element",
  "--color-text", "--color-text-muted",
  "--color-border", "--color-border-active", "--color-border-subtle",
  "--color-error", "--color-warning", "--color-success", "--color-info",
];

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
  // Selection is a name, and it starts as whatever is actually applied. The
  // old index-based cursor defaulted to 0 and a preview effect repainted the
  // app with theme #0 the moment the async list landed — so simply opening the
  // picker after a reload looked like the theme had reset.
  const [activeName, setActiveName] = useState(activeThemeName);
  const [cursor, setCursor] = useState(activeThemeName);
  const [applying, setApplying] = useState(false);
  const [localAppearance, setLocalAppearance] = useState<Appearance>(appearance);
  const inputRef = useRef<HTMLInputElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useFocusTrap(modalRef);

  // Save originals for revert on cancel.
  const originalTheme = useRef<Record<string, string>>({});
  const originalMode = useRef<ThemeMode>(themeMode);
  const originalAppearance = useRef<Appearance>(appearance);

  useEffect(() => {
    const root = document.documentElement;
    const saved: Record<string, string> = {};
    for (const v of PREVIEW_VARS) saved[v] = getComputedStyle(root).getPropertyValue(v).trim();
    originalTheme.current = saved;
    originalMode.current = themeMode;
    originalAppearance.current = appearance;

    fetchThemes()
      .then((data) => {
        setThemes(data);
        setLoading(false);
        // The stored pair may not have arrived when the modal mounted.
        const live = activeThemeName();
        if (live) {
          setActiveName(live);
          setCursor((c) => c || live);
        }
      })
      .catch(() => setLoading(false));

    inputRef.current?.focus();
  }, []);

  const filtered = useMemo(() => {
    if (!filter) return themes;
    const lower = filter.toLowerCase();
    return themes.filter((t) => t.name.toLowerCase().includes(lower));
  }, [themes, filter]);

  const colorsFor = useCallback(
    (theme: ThemePreview) => previewColors(theme, localAppearance),
    [localAppearance],
  );

  /** Move the cursor and preview in one step — previewing is never implicit. */
  const preview = useCallback((theme: ThemePreview) => {
    setCursor(theme.name);
    applyThemeToCss(previewColors(theme, localAppearance));
  }, [localAppearance]);

  const revertAndClose = useCallback(() => {
    const root = document.documentElement;
    for (const [k, v] of Object.entries(originalTheme.current)) root.style.setProperty(k, v);
    if (themeMode !== originalMode.current) {
      onThemeModeChange(originalMode.current);
      applyThemeMode(originalMode.current);
      persistThemeMode(originalMode.current);
    }
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
      setCursor(theme.name);
      const commit = (colors: ThemeColors) => {
        applyThemeToCss(colors);
        onThemeApplied(colors);
        onAppearanceChange(localAppearance);
        persistAppearance(localAppearance);
        applyAppearanceClass(localAppearance);
        setActiveName(theme.name);
        onClose();
      };
      try {
        const pair = await switchTheme(theme.name);
        // Older servers answer without a name; the picked one is authoritative.
        const named = { ...pair, name: pair.name || theme.name };
        storeThemePair(named, localAppearance);
        commit(resolveThemeColors(named, localAppearance));
      } catch {
        commit(previewColors(theme, localAppearance));
      } finally {
        setApplying(false);
      }
    },
    [onClose, onThemeApplied, localAppearance, onAppearanceChange],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        revertAndClose();
        return;
      }
      const idx = filtered.findIndex((t) => t.name === cursor);
      if (e.key === "Enter") {
        e.preventDefault();
        const target = filtered[idx] ?? filtered[0];
        if (target) handleApplyTheme(target);
        return;
      }
      const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
      if (step === 0) return;
      e.preventDefault();
      const next = filtered[Math.min(Math.max(idx + step, 0), filtered.length - 1)];
      if (next) preview(next);
    },
    [filtered, cursor, handleApplyTheme, preview, revertAndClose],
  );

  const handleAppearanceSwitch = useCallback((a: Appearance) => {
    setLocalAppearance(a);
    applyAppearanceClass(a);
    // Repaint the palette on the cursor so light/dark preview stays truthful.
    const shown = themes.find((t) => t.name === (cursor || activeName));
    if (shown) applyThemeToCss(previewColors(shown, a));
  }, [themes, cursor, activeName]);

  const handleModeSwitch = useCallback(
    (mode: ThemeMode) => { onThemeModeChange(mode); applyThemeMode(mode); persistThemeMode(mode); },
    [onThemeModeChange],
  );

  return (
    <div className="modal-backdrop" onClick={revertAndClose}>
      <div
        className="theme-selector"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        role="dialog" aria-modal="true" aria-label="Appearance settings" ref={modalRef}
      >
        <div className="theme-selector-header">
          <Palette size={14} />
          <span>Appearance</span>
          <button className="theme-selector-close" onClick={revertAndClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        <AppearanceSwitcher value={localAppearance} onChange={handleAppearanceSwitch} />
        <ModeSwitcher value={themeMode} onChange={handleModeSwitch} />

        <div className="theme-section-label">
          <span>Color Themes</span>
          <span className="theme-selector-count">{filtered.length}</span>
        </div>

        <div className="theme-selector-search">
          <Search size={13} />
          <input ref={inputRef} className="theme-selector-input" type="text"
            placeholder="Search themes..." value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>

        <ThemeGrid
          themes={filtered} loading={loading} cursor={cursor} activeName={activeName}
          applying={applying} colorsFor={colorsFor} onHover={preview} onPick={handleApplyTheme}
        />

        <div className="theme-selector-footer">
          <kbd>Up/Down</kbd> Navigate <kbd>Enter</kbd> Apply <kbd>Esc</kbd> Cancel
        </div>
      </div>
    </div>
  );
}
