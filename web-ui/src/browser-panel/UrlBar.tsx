import React, { useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Keyboard,
  Power,
  RotateCw,
  SquareArrowOutUpRight,
} from "lucide-react";

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
  readonly onGo: (url: string) => void;
  readonly onBack: () => void;
  readonly onForward: () => void;
  readonly onReload: () => void;
  readonly onEndSession: () => void;
  /** True while the page has the keyboard and the app's chords are standing down. */
  readonly capturing: boolean;
}

export const UrlBar: React.FC<UrlBarProps> = React.memo(function UrlBar({
  url,
  loading,
  onGo,
  onBack,
  onForward,
  onReload,
  onEndSession,
  capturing,
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
        const value = draft.trim();
        if (!value) return;
        const target = /^(https?:\/\/|about:blank$)/i.test(value) ? value
          : /^(localhost|[\w-]+(?:\.[\w-]+)+)(:\d+)?([/?#].*)?$/.test(value)
            ? `http${value.startsWith("localhost") ? "" : "s"}://${value}`
            : `https://www.google.com/search?q=${encodeURIComponent(value)}`;
        onGo(target);
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
        placeholder="Search or enter a URL"
        aria-label="Address"
        onChange={(event) => setDraft(event.target.value)}
        onFocus={(event) => {
          setEditing(true);
          event.target.select();
        }}
        onBlur={() => setEditing(false)}
      />

      {capturing ? (
        <span
          className="bwp-capture"
          title="This page has the keyboard. Press Escape twice to give it back to the app."
        >
          <Keyboard size={12} />
          <span className="bwp-capture-hint">esc esc</span>
        </span>
      ) : null}

      <button
        type="button"
        className="bwp-nav"
        onClick={onEndSession}
        title="Restart this tab"
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
