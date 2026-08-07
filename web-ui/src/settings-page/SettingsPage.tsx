import React, { Suspense, lazy } from "react";
import { ArrowLeft } from "lucide-react";
import type { ThemeColors } from "../api";
import type { Appearance } from "../utils/appearance";
import type { ThemeMode } from "../theme-selector/persistence";
import { KeybindingsPanel } from "../keybindings-view/KeybindingsPanel";
import { AppearanceSection } from "./AppearanceSection";
import { SECTIONS } from "./sections";
import type { SettingsSection } from "./useSettingsRoute";

// The two sections that fetch on mount are split out: opening Appearance should not pull
// the MCP and skills editors, and neither is on the path to first paint.
const ServersSection = lazy(() =>
  import("./mcp/ServersSection").then((m) => ({ default: m.ServersSection })),
);
const SkillsSection = lazy(() =>
  import("./skills/SkillsSection").then((m) => ({ default: m.SkillsSection })),
);

/**
 * Settings: one destination for how opman is configured.
 *
 * Four editors — appearance, keybindings, MCP servers, skills — reached by a rail rather
 * than a stack of modals. What lives here is configuration; what opman *remembers*
 * (routines, session instructions, memory) is content and keeps its own surfaces.
 */

export interface SettingsPageProps {
  readonly section: SettingsSection;
  readonly onSelectSection: (section: SettingsSection) => void;
  /** Return to the conversation the user came from. */
  readonly onExit: () => void;
  readonly appearance: Appearance;
  readonly onAppearanceChange: (appearance: Appearance) => void;
  readonly themeMode: ThemeMode;
  readonly onThemeModeChange: (mode: ThemeMode) => void;
  readonly onThemeApplied: (colors: ThemeColors) => void;
  readonly onError: (message: string) => void;
  /** Runner slots on offer, so an MCP server can be scoped to any of them. */
  readonly runners: readonly string[];
}

/** Keep the active rail item in view without a layout-thrashing effect. */
function revealRailItem(el: HTMLButtonElement | null) {
  el?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

export function SettingsPage(props: SettingsPageProps) {
  const { section, onSelectSection, onExit } = props;
  const current = SECTIONS.find((meta) => meta.id === section) ?? SECTIONS[0];

  return (
    <div className="stg-page" data-surface="settings">
      <header className="stg-head">
        <button type="button" className="stg-back" onClick={onExit}>
          <ArrowLeft size={14} aria-hidden="true" />
          <span>Back to chat</span>
        </button>
        <h1 className="stg-title">Settings</h1>
        <span className="stg-head-note">~/.config/opman</span>
      </header>

      <div className="stg-body">
        <nav className="stg-rail" aria-label="Settings sections">
          {SECTIONS.map((meta) => (
            <button
              key={meta.id}
              type="button"
              className={meta.id === section ? "stg-rail-item is-active" : "stg-rail-item"}
              aria-current={meta.id === section ? "page" : undefined}
              // On a narrow viewport the rail is a scrolling row, and the section on
              // screen can start off it. `nearest` on both axes keeps that from moving
              // the pane as well.
              ref={meta.id === section ? revealRailItem : undefined}
              onClick={() => onSelectSection(meta.id)}
            >
              <span className="stg-rail-icon" aria-hidden="true">
                {meta.icon}
              </span>
              <span className="stg-rail-text">
                <span className="stg-rail-label">{meta.label}</span>
                <span className="stg-rail-summary">{meta.summary}</span>
              </span>
            </button>
          ))}
        </nav>

        <main className="stg-pane" aria-label={current.label}>
          <div className="stg-pane-head">
            <h2 className="stg-pane-title">{current.label}</h2>
          </div>
          <div className="stg-pane-body">
            <Suspense fallback={<div className="stg-loading">Loading…</div>}>
              {section === "appearance" && (
                <AppearanceSection
                  appearance={props.appearance}
                  onAppearanceChange={props.onAppearanceChange}
                  themeMode={props.themeMode}
                  onThemeModeChange={props.onThemeModeChange}
                  onThemeApplied={props.onThemeApplied}
                />
              )}
              {section === "keybindings" && <KeybindingsPanel />}
              {section === "mcp" && (
                <ServersSection onError={props.onError} runners={props.runners} />
              )}
              {section === "skills" && <SkillsSection onError={props.onError} />}
            </Suspense>
          </div>
        </main>
      </div>
    </div>
  );
}
