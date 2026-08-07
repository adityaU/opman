import React from "react";
import { Loader2, Layers, Square, Monitor, Sun, Moon } from "lucide-react";
import type { ThemePreview, ThemeColors } from "../api";
import type { Appearance } from "../utils/appearance";
import type { ThemeMode } from "./persistence";

const APPEARANCES = [
  ["system", Monitor, "System", "Follow OS"],
  ["light", Sun, "Light", "Light colors"],
  ["dark", Moon, "Dark", "Dark colors"],
] as const;

export function AppearanceSwitcher({
  value,
  onChange,
}: {
  value: Appearance;
  onChange: (a: Appearance) => void;
}) {
  return (
    <div className="theme-appearance-switcher">
      {APPEARANCES.map(([key, Icon, label, desc]) => (
        <button
          key={key}
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

const MODES = [
  ["glassy", Layers, "Glassy", "Translucent blur"],
  ["flat", Square, "Flat", "Solid surfaces"],
] as const;

export function ModeSwitcher({
  value,
  onChange,
}: {
  value: ThemeMode;
  onChange: (m: ThemeMode) => void;
}) {
  return (
    <div className="theme-mode-switcher">
      {MODES.map(([key, Icon, label, desc]) => (
        <button
          key={key}
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

interface GridProps {
  themes: ThemePreview[];
  loading: boolean;
  /** Name of the theme the cursor is on — never an index, so filtering the
   *  list cannot silently move the selection to a different palette. */
  cursor: string;
  /** Name of the palette actually applied to the app right now. */
  activeName: string;
  applying: boolean;
  colorsFor: (theme: ThemePreview) => ThemeColors;
  onHover: (theme: ThemePreview) => void;
  onPick: (theme: ThemePreview) => void;
}

export function ThemeGrid({
  themes, loading, cursor, activeName, applying, colorsFor, onHover, onPick,
}: GridProps) {
  if (loading) {
    return (
      <div className="theme-selector-grid">
        <div className="theme-selector-loading">
          <Loader2 size={16} className="spinning" />
          <span>Loading themes...</span>
        </div>
      </div>
    );
  }
  if (themes.length === 0) {
    return <div className="theme-selector-grid"><div className="theme-selector-empty">No themes found</div></div>;
  }

  return (
    <div className="theme-selector-grid">
      {themes.map((theme) => {
        const c = colorsFor(theme);
        const isCursor = theme.name === cursor;
        const isActive = theme.name === activeName;
        return (
          <button
            key={theme.name}
            className={`theme-card${isCursor ? " selected" : ""}${isActive ? " active" : ""}`}
            aria-current={isActive || undefined}
            onClick={() => onPick(theme)}
            onMouseEnter={() => onHover(theme)}
            ref={isCursor ? scrollIntoView : undefined}
          >
            <div className="theme-card-preview">
              <span style={{ background: c.background, flex: 2 }} />
              <span style={{ background: c.primary, flex: 1 }} />
              <span style={{ background: c.secondary, flex: 1 }} />
              <span style={{ background: c.accent, flex: 1 }} />
              <span style={{ background: c.text, flex: 1 }} />
            </div>
            <span className="theme-card-name">{theme.name}</span>
            {applying && isCursor && <Loader2 size={12} className="spinning" />}
          </button>
        );
      })}
    </div>
  );
}

/** Keep the keyboard cursor visible without a layout-thrashing effect. */
function scrollIntoView(el: HTMLButtonElement | null) {
  el?.scrollIntoView({ block: "nearest" });
}
