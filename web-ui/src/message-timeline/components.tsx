import React from "react";
import { FolderOpen, Cpu } from "lucide-react";
import { EXAMPLE_PROMPTS } from "./types";

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

/** Welcome screen when no session is selected */
export function WelcomeEmpty() {
  return (
    <div className="message-timeline-empty">
      <div className="message-timeline-welcome">
        <h2>Welcome to OpenCode</h2>
        <p>Select a session from the sidebar or create a new one to start chatting.</p>
        <div className="message-timeline-shortcuts">
          <kbd>Cmd+Shift+N</kbd> New Session
          <kbd>Cmd+Shift+P</kbd> Command Palette
          <kbd>Cmd&apos;</kbd> Model Picker
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
