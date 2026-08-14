import React from "react";
import { FolderOpen, Cpu, Loader2, CircleSlash } from "lucide-react";
import { EXAMPLE_PROMPTS } from "./types";
import { OpmanMark } from "../OpmanMark";
import type { CommandId } from "../keybindings/types";
import { useChordLabeller } from "../keybindings/useChord";

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
      <div className="pending-reply-inner" title={detail}>
        {settled
          ? <CircleSlash size={13} className="pending-reply-icon" />
          : <Loader2 size={13} className="tool-spin-icon" />}
        <span className="pending-reply-label">{label}</span>
        {settled ? <span className="pending-reply-detail">{detail}</span> : null}
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

/**
 * The three keys worth learning on day one.
 *
 * Named as commands, not as chords: this is the first thing a new user reads,
 * so it is the last place that should be guessing at their platform or their
 * keymap mode.
 */
const WELCOME_SHORTCUTS: readonly { readonly command: CommandId; readonly label: string }[] = [
  { command: "session.new", label: "Start a session" },
  { command: "palette.commands", label: "Command palette" },
  { command: "engine.model", label: "Switch model" },
];

/** Welcome screen when no session is selected — the product's front door. */
export function WelcomeEmpty() {
  const chordFor = useChordLabeller();
  const shortcuts = WELCOME_SHORTCUTS.map(({ command, label }) => [label, chordFor(command)] as const)
    .filter((entry): entry is readonly [string, string] => entry[1] !== undefined);
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

const SESSION_SHORTCUTS: readonly { readonly command: CommandId; readonly label: string }[] = [
  { command: "engine.model", label: "Model Picker" },
  { command: "layout.toggleEditor", label: "Editor" },
  { command: "layout.toggleGit", label: "Git" },
];

/** The chord strip under a fresh session's prompt cards. */
function SessionShortcutRow() {
  const chordFor = useChordLabeller();
  return (
    <div className="message-timeline-shortcuts">
      {SESSION_SHORTCUTS.map(({ command, label }) => {
        const chord = chordFor(command);
        return chord ? (
          <span key={command}>
            <kbd>{chord}</kbd> {label}
          </span>
        ) : null;
      })}
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

        <SessionShortcutRow />
      </div>
    </div>
  );
}
