import React from "react";
import { FolderOpen, Cpu, Loader2, CircleSlash } from "lucide-react";
import { EXAMPLE_PROMPTS } from "./types";
import { OpmanMark } from "../OpmanMark";

/**
 * Placeholder for a turn that is under way but has produced nothing visible
 * yet. `claude --bg` can take several seconds to spawn and write its first
 * token; without this the transcript reads as stalled or, on a session's first
 * send, as empty.
 */
export function PendingReply({
  label,
  detail,
  settled = false,
}: {
  label: string;
  detail: string;
  /** The turn is over — render a verdict rather than a spinner, which would
   *  otherwise keep implying work is still happening. */
  settled?: boolean;
}) {
  return (
    <div className={`pending-reply${settled ? " pending-reply-settled" : ""}`} role="status" aria-live="polite">
      <div className="pending-reply-inner">
        {settled
          ? <CircleSlash size={15} className="pending-reply-icon" />
          : <Loader2 size={15} className="tool-spin-icon" />}
        <div className="pending-reply-text">
          <span className="pending-reply-label">{label}</span>
          <span className="pending-reply-detail">{detail}</span>
        </div>
      </div>
    </div>
  );
}

/** Shimmer skeleton shown while messages are loading. */
export function MessageShimmer() {
  return (
    <div className="message-shimmer" aria-label="Loading messages">
      <div className="shimmer-turn shimmer-user">
        <div className="shimmer-content">
          <div className="shimmer-header-row">
            <div className="shimmer-avatar" />
            <div className="shimmer-line shimmer-role-label" />
          </div>
          <div className="shimmer-line shimmer-w-55" />
          <div className="shimmer-line shimmer-w-35" />
        </div>
      </div>
      <div className="shimmer-turn shimmer-assistant">
        <div className="shimmer-content">
          <div className="shimmer-header-row">
            <div className="shimmer-avatar" />
            <div className="shimmer-line shimmer-role-label" />
          </div>
          <div className="shimmer-line shimmer-w-90" />
          <div className="shimmer-line shimmer-w-75" />
          <div className="shimmer-line shimmer-w-60" />
          <div className="shimmer-line shimmer-w-45" />
        </div>
      </div>
      <div className="shimmer-turn shimmer-user">
        <div className="shimmer-content">
          <div className="shimmer-header-row">
            <div className="shimmer-avatar" />
            <div className="shimmer-line shimmer-role-label" />
          </div>
          <div className="shimmer-line shimmer-w-40" />
        </div>
      </div>
      <div className="shimmer-turn shimmer-assistant">
        <div className="shimmer-content">
          <div className="shimmer-header-row">
            <div className="shimmer-avatar" />
            <div className="shimmer-line shimmer-role-label" />
          </div>
          <div className="shimmer-line shimmer-w-80" />
          <div className="shimmer-line shimmer-w-65" />
          <div className="shimmer-line shimmer-w-50" />
        </div>
      </div>
    </div>
  );
}

/** True on macOS-family platforms, where the modifier key is ⌘ rather than Ctrl. */
const IS_MAC = /Mac|iPhone|iPad/.test(
  typeof navigator !== "undefined" ? navigator.platform : "",
);

/** Welcome screen when no session is selected — the product's front door. */
export function WelcomeEmpty() {
  const shortcuts: Array<[string, string]> = IS_MAC
    ? [
        ["Start a session", "⌘⇧N"],
        ["Command palette", "⌘⇧P"],
        ["Switch model", "⌘'"],
      ]
    : [
        ["Start a session", "Ctrl+Shift+N"],
        ["Command palette", "Ctrl+Shift+P"],
        ["Switch model", "Ctrl+'"],
      ];
  return (
    <div className="message-timeline-empty">
      <div className="message-timeline-welcome home-welcome">
        <div className="home-welcome-halo" aria-hidden="true" />
        <OpmanMark size={56} className="home-welcome-mark" />
        <h2 className="home-welcome-title">Opman</h2>
        <p className="home-welcome-tagline">
          Mission control for your coding agents. Pick a session from the
          sidebar, or start a new one and put an agent to work.
        </p>
        <div className="message-timeline-shortcuts home-welcome-shortcuts">
          {shortcuts.map(([label, keys]) => (
            <div className="home-shortcut" key={label}>
              <span className="home-shortcut-label">{label}</span>
              <kbd>{keys}</kbd>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/** New session empty state with example prompts */
export function NewSessionEmpty({
  sessionDirectory,
  defaultModel,
  onSendPrompt,
}: {
  sessionDirectory: string | null;
  defaultModel?: string | null;
  onSendPrompt?: (text: string) => void;
}) {
  return (
    <div className="message-timeline-empty">
      <div className="message-timeline-welcome new-session-welcome">
        <h2>New Session</h2>

        <div className="new-session-info">
          {sessionDirectory && (
            <div className="new-session-info-row">
              <FolderOpen size={14} />
              <span className="new-session-directory" title={sessionDirectory}>
                {sessionDirectory}
              </span>
            </div>
          )}
          {defaultModel && (
            <div className="new-session-info-row">
              <Cpu size={14} />
              <span className="new-session-model-badge">{defaultModel}</span>
            </div>
          )}
        </div>

        <p>Type a message below or try one of these:</p>

        <div className="new-session-prompts">
          {EXAMPLE_PROMPTS.map((prompt, i) => (
            <button
              key={i}
              className="new-session-prompt-card"
              onClick={() => onSendPrompt?.(prompt.text)}
            >
              <prompt.icon size={16} className="new-session-prompt-icon" />
              <span>{prompt.text}</span>
            </button>
          ))}
        </div>

        <div className="message-timeline-shortcuts">
          <kbd>Cmd&apos;</kbd> Model Picker
          <kbd>Cmd+Shift+E</kbd> Editor
          <kbd>Cmd+Shift+G</kbd> Git
        </div>
      </div>
    </div>
  );
}
