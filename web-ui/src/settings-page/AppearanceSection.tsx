import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AlignJustify, Rows2, Rows3, Search } from "lucide-react";
import { fetchThemes, switchTheme } from "../api";
import type { ThemeColors, ThemePreview } from "../api";
import { applyThemeToCss } from "../utils/theme";
import {
  activeThemeName,
  applyAppearanceClass,
  persistAppearance,
  resolveAppearance,
  resolveThemeColors,
  storeThemePair,
} from "../utils/appearance";
import type { Appearance } from "../utils/appearance";
import { AppearanceSwitcher, ModeSwitcher, ThemeGrid } from "../theme-selector/controls";
import { applyThemeMode, persistThemeMode } from "../theme-selector/persistence";
import type { ThemeMode } from "../theme-selector/persistence";
import { applyDensity, loadDensity, persistDensity } from "../utils/density";
import type { Density } from "../utils/density";

/**
 * Density picker.
 *
 * Deliberately the same three-tile control as Surfaces above it rather than a
 * slider: the useful range is three steps wide, and a slider would imply a
 * continuum the layout does not actually have.
 */
const DENSITIES: readonly [Density, typeof Rows3, string, string][] = [
  ["compact", Rows3, "Compact", "6px between surfaces"],
  ["default", Rows2, "Default", "12px between surfaces"],
  ["roomy", AlignJustify, "Roomy", "20px between surfaces"],
];

function DensitySwitcher({
  value,
  onChange,
}: {
  value: Density;
  onChange: (density: Density) => void;
}) {
  return (
    <div className="theme-mode-switcher">
      {DENSITIES.map(([key, Icon, label, desc]) => (
        <button
          key={key}
          type="button"
          className={`theme-mode-option${value === key ? " active" : ""}`}
          aria-pressed={value === key}
          onClick={() => onChange(key)}
        >
          <Icon size={16} />
          <div className="theme-mode-text">
            <span className="theme-mode-label">{label}</span>
            <span className="theme-mode-desc">{desc}</span>
          </div>
        </button>
      ))}
    </div>
  );
}

/**
 * Theme, light/dark, and glassy/flat.
 *
 * The same controls the quick picker used, with page semantics instead of dialog ones:
 * there is no Cancel to revert to, so hovering previews and leaving the grid puts the
 * applied palette back. Picking one is the commit.
 */

export interface AppearanceSectionProps {
  readonly appearance: Appearance;
  readonly onAppearanceChange: (appearance: Appearance) => void;
  readonly themeMode: ThemeMode;
  readonly onThemeModeChange: (mode: ThemeMode) => void;
  readonly onThemeApplied: (colors: ThemeColors) => void;
}

function previewColors(theme: ThemePreview, appearance: Appearance): ThemeColors {
  return resolveAppearance(appearance) === "light" ? theme.light : theme.dark;
}

export function AppearanceSection(props: AppearanceSectionProps) {
  const { appearance, onAppearanceChange, themeMode, onThemeModeChange, onThemeApplied } = props;
  const [themes, setThemes] = useState<ThemePreview[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [activeName, setActiveName] = useState(activeThemeName);
  const [cursor, setCursor] = useState(activeThemeName);
  const [applying, setApplying] = useState(false);
  // Selection is a name, never an index: filtering the list must not silently move the
  // cursor onto a different palette.
  const activeRef = useRef(activeName);
  activeRef.current = activeName;

  useEffect(() => {
    let alive = true;
    fetchThemes()
      .then((list) => {
        if (!alive) return;
        setThemes(list);
        setLoading(false);
        // The stored pair may not have landed when this mounted.
        const live = activeThemeName();
        if (!live) return;
        setActiveName(live);
        setCursor((current) => current || live);
      })
      .catch(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, []);

  const filtered = useMemo(() => {
    if (!filter) return themes;
    const needle = filter.toLowerCase();
    return themes.filter((theme) => theme.name.toLowerCase().includes(needle));
  }, [themes, filter]);

  const colorsFor = useCallback(
    (theme: ThemePreview) => previewColors(theme, appearance),
    [appearance],
  );

  const preview = useCallback(
    (theme: ThemePreview) => {
      setCursor(theme.name);
      applyThemeToCss(previewColors(theme, appearance));
    },
    [appearance],
  );

  /** Put the applied palette back once the pointer leaves the grid. */
  const endPreview = useCallback(() => {
    const applied = themes.find((theme) => theme.name === activeRef.current);
    setCursor(activeRef.current);
    if (applied) applyThemeToCss(previewColors(applied, appearance));
  }, [themes, appearance]);

  const apply = useCallback(
    async (theme: ThemePreview) => {
      setApplying(true);
      setCursor(theme.name);
      const commit = (colors: ThemeColors) => {
        applyThemeToCss(colors);
        onThemeApplied(colors);
        setActiveName(theme.name);
      };
      try {
        const pair = await switchTheme(theme.name);
        // Older servers answer without a name; the palette the user clicked is
        // authoritative, and without this the picker resets to theme #0 on reload.
        const named = { ...pair, name: pair.name || theme.name };
        storeThemePair(named, appearance);
        commit(resolveThemeColors(named, appearance));
      } catch {
        commit(previewColors(theme, appearance));
      } finally {
        setApplying(false);
      }
    },
    [appearance, onThemeApplied],
  );

  const changeAppearance = useCallback(
    (next: Appearance) => {
      onAppearanceChange(next);
      applyAppearanceClass(next);
      persistAppearance(next);
      // Repaint on the shown palette so a light/dark switch stays truthful.
      const shown = themes.find((theme) => theme.name === (cursor || activeName));
      if (shown) applyThemeToCss(previewColors(shown, next));
    },
    [onAppearanceChange, themes, cursor, activeName],
  );

  const changeMode = useCallback(
    (mode: ThemeMode) => {
      onThemeModeChange(mode);
      applyThemeMode(mode);
      persistThemeMode(mode);
    },
    [onThemeModeChange],
  );

  // Density is local state: unlike the palette, nothing else in React needs to
  // know it — it lives on <html> as a custom property and the cascade does the
  // rest, so lifting it into App would buy nothing.
  const [density, setDensity] = useState<Density>(loadDensity);

  const changeDensity = useCallback((next: Density) => {
    setDensity(next);
    applyDensity(next);
    persistDensity(next);
  }, []);

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <h3 className="stg-card-title">Light and dark</h3>
        <p className="stg-card-note">Every palette ships both. System follows the OS.</p>
        <AppearanceSwitcher value={appearance} onChange={changeAppearance} />
      </section>

      <section className="stg-card">
        <h3 className="stg-card-title">Surfaces</h3>
        <p className="stg-card-note">
          Glassy blurs what is behind a panel; flat paints it solid. Both are fully
          supported everywhere — pick by taste, or by what your machine renders fastest.
        </p>
        <ModeSwitcher value={themeMode} onChange={changeMode} />
      </section>

      <section className="stg-card">
        <h3 className="stg-card-title">Density</h3>
        <p className="stg-card-note">
          One number sets every gap in the shell: the space around the app, between the
          sidebar and your panes, and between two panes.
        </p>
        <DensitySwitcher value={density} onChange={changeDensity} />
      </section>

      <section className="stg-card">
        <div className="stg-card-head">
          <h3 className="stg-card-title">Color theme</h3>
          <span className="stg-count">{filtered.length}</span>
        </div>
        <label className="stg-search">
          <Search size={13} aria-hidden="true" />
          <input
            type="search"
            placeholder="Search themes"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            aria-label="Search themes"
          />
        </label>
        <div onMouseLeave={endPreview}>
          <ThemeGrid
            themes={filtered}
            loading={loading}
            cursor={cursor}
            activeName={activeName}
            applying={applying}
            colorsFor={colorsFor}
            onHover={preview}
            onPick={apply}
          />
        </div>
        <p className="stg-card-note">Hover to preview, click to apply.</p>
      </section>
    </div>
  );
}
