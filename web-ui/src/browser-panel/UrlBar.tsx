import React, { useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Monitor,
  Power,
  RotateCw,
  SquareArrowOutUpRight,
} from "lucide-react";
import type { BrowserMode } from "../api/browser";

/**
 * The pane's address bar.
 *
 * Deliberately one row: a browser pane is usually a third of a screen wide, and
 * a second toolbar row would cost more of the page than it returns. The mode
 * toggle sits with the other controls rather than in a menu because it is the
 * one thing a user reaches for when a site renders wrong.
 */

interface UrlBarProps {
  readonly url: string;
  readonly loading: boolean;
  readonly mode: BrowserMode;
  readonly onGo: (url: string) => void;
  readonly onBack: () => void;
  readonly onForward: () => void;
  readonly onReload: () => void;
  readonly onToggleMode: () => void;
  readonly onEndSession: () => void;
}

export const UrlBar: React.FC<UrlBarProps> = React.memo(function UrlBar({
  url,
  loading,
  mode,
  onGo,
  onBack,
  onForward,
  onReload,
  onToggleMode,
  onEndSession,
}) {
  const [draft, setDraft] = useState(url);
  const [editing, setEditing] = useState(false);

  // Adopt the real URL whenever the page moves — including navigations the
  // agent made — but never while the user is mid-edit.
  useEffect(() => {
    if (!editing) setDraft(url);
  }, [url, editing]);

  return (
    <form
      className="bwp-urlbar"
      onSubmit={(event) => {
        event.preventDefault();
        setEditing(false);
        onGo(draft);
      }}
    >
      <button type="button" className="bwp-nav" onClick={onBack} title="Back" aria-label="Back">
        <ArrowLeft size={14} />
      </button>
      <button
        type="button"
        className="bwp-nav"
        onClick={onForward}
        title="Forward"
        aria-label="Forward"
      >
        <ArrowRight size={14} />
      </button>
      <button
        type="button"
        className={`bwp-nav${loading ? " is-loading" : ""}`}
        onClick={onReload}
        title="Reload"
        aria-label="Reload"
      >
        <RotateCw size={14} />
      </button>

      <input
        className="bwp-url"
        value={draft}
        spellCheck={false}
        autoComplete="off"
        placeholder="Enter a URL"
        aria-label="Address"
        onChange={(event) => setDraft(event.target.value)}
        onFocus={(event) => {
          setEditing(true);
          event.target.select();
        }}
        onBlur={() => setEditing(false)}
      />

      <button
        type="button"
        className={`bwp-nav${mode === "screencast" ? " is-active" : ""}`}
        onClick={onToggleMode}
        title={
          mode === "screencast"
            ? "Mirroring the page — click to try an embedded frame"
            : "Embedded frame — click to mirror the real page instead"
        }
        aria-label="Toggle rendering mode"
        aria-pressed={mode === "screencast"}
      >
        <Monitor size={14} />
      </button>
      <button
        type="button"
        className="bwp-nav"
        onClick={onEndSession}
        title="End this browser session — clears cookies and history for the project"
        aria-label="End browser session"
      >
        <Power size={14} />
      </button>
      <a
        className="bwp-nav"
        href={url || undefined}
        target="_blank"
        rel="noreferrer noopener"
        title="Open in a real browser tab"
        aria-label="Open externally"
      >
        <SquareArrowOutUpRight size={14} />
      </a>
    </form>
  );
});
