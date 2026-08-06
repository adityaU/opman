/**
 * ComposerBar — the composer's writing surface and its action rail.
 *
 * The message is the point, so the textarea owns the full width of the card and
 * every control sits on one baseline underneath it: context on the left (see
 * SelectorChips), actions on the right. Growing text pushes the rail down
 * instead of squeezing between two fixed gutters.
 */
import React from "react";
import { ArrowUp, Loader2, Paperclip, Square, SquareTerminal } from "lucide-react";
import { RUNNER_LABELS } from "./helpers";

// ── ComposerField ───────────────────────────────────────────────

interface ComposerFieldProps {
  textareaRef: React.RefObject<HTMLTextAreaElement>;
  fileInputRef: React.RefObject<HTMLInputElement>;
  text: string;
  disabled: boolean;
  isBusy: boolean;
  runner: string;
  onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onPaste: (e: React.ClipboardEvent<HTMLTextAreaElement>) => void;
  onFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

/** Name the destination rather than the mechanics — the rail teaches the rest. */
function placeholderFor(disabled: boolean, isBusy: boolean, runner: string): string {
  if (disabled) return "Select a session to start";
  if (isBusy) return "Queue a follow-up…";
  return `Message ${RUNNER_LABELS[runner] || runner}…`;
}

export function ComposerField({
  textareaRef, fileInputRef, text, disabled, isBusy, runner,
  onChange, onKeyDown, onPaste, onFileSelect,
}: ComposerFieldProps) {
  return (
    <div className="composer-field">
      <input ref={fileInputRef} type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/bmp"
        multiple onChange={onFileSelect} style={{ display: "none" }} />
      <textarea ref={textareaRef} className="prompt-textarea" value={text}
        onChange={onChange} onKeyDown={onKeyDown} onPaste={onPaste}
        placeholder={placeholderFor(disabled, isBusy, runner)}
        disabled={disabled} rows={1} />
    </div>
  );
}

// ── ComposerActions ─────────────────────────────────────────────

interface ComposerActionsProps {
  disabled: boolean;
  isBusy: boolean;
  isSending?: boolean;
  hasContent: boolean;
  onAttachClick: () => void;
  onSlashClick: () => void;
  onSubmit: () => void;
  onAbort: () => void;
  /** Claude engine only: open an interactive terminal attached to this session. */
  onAttachTerminal?: () => void;
}

export function ComposerActions({
  disabled, isBusy, isSending, hasContent,
  onAttachClick, onSlashClick, onSubmit, onAbort, onAttachTerminal,
}: ComposerActionsProps) {
  return (
    <div className="composer-actions">
      {/* The character you would type, offered as a target — not an icon
          standing in for one. */}
      <button type="button" className="composer-icon-btn composer-key-btn" onClick={onSlashClick}
        disabled={disabled} title="Run a command" aria-label="Run a command">
        /
      </button>
      <button type="button" className="composer-icon-btn" onClick={onAttachClick}
        disabled={disabled} title="Attach an image — you can also paste or drop one"
        aria-label="Attach an image">
        <Paperclip size={14} />
      </button>
      {onAttachTerminal && (
        <button type="button" className="composer-icon-btn" onClick={onAttachTerminal}
          disabled={disabled} title="Open the claude CLI attached to this session"
          aria-label="Attach terminal">
          <SquareTerminal size={14} />
        </button>
      )}
      {isBusy && (
        <button type="button" className="composer-stop" onClick={onAbort}
          title="Stop generating" aria-label="Stop generating">
          <Square size={13} fill="currentColor" />
        </button>
      )}
      {isSending ? (
        <button type="button" className="composer-send" disabled title="Sending…" aria-label="Sending message">
          <Loader2 size={16} className="spinning" />
        </button>
      ) : (
        <button type="button" className="composer-send" onClick={onSubmit}
          disabled={disabled || !hasContent}
          title={isBusy ? "Queue follow-up (Enter)" : "Send (Enter)"} aria-label="Send message">
          <ArrowUp size={17} strokeWidth={2.4} />
        </button>
      )}
    </div>
  );
}
