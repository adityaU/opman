import React, {
  useState,
  useRef,
  useCallback,
  useEffect,
  type KeyboardEvent,
} from "react";
import { Send, Square, Cpu, ChevronDown, Loader2 } from "lucide-react";
import { SlashCommandPopover } from "./SlashCommandPopover";

interface Props {
  onSend: (text: string) => void;
  onAbort: () => void;
  onCommand: (command: string, args?: string) => void;
  onOpenModelPicker: () => void;
  isBusy: boolean;
  isSending?: boolean;
  disabled: boolean;
  sessionId: string | null;
  currentModel: string | null;
}

export function PromptInput({
  onSend,
  onAbort,
  onCommand,
  onOpenModelPicker,
  isBusy,
  isSending,
  disabled,
  sessionId,
  currentModel,
}: Props) {
  const [text, setText] = useState("");
  const [showSlash, setShowSlash] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [text]);

  // Focus input on mount and when session changes
  useEffect(() => {
    textareaRef.current?.focus();
  }, [sessionId]);

  const handleSubmit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed) return;

    // Check for slash commands
    if (trimmed.startsWith("/")) {
      const parts = trimmed.split(/\s+/);
      const cmd = parts[0].slice(1); // Remove leading /
      const args = parts.slice(1).join(" ");
      onCommand(cmd, args);
      setText("");
      return;
    }

    onSend(trimmed);
    setText("");
  }, [text, onSend, onCommand]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter sends, Shift+Enter for newline
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        if (isBusy) return;
        handleSubmit();
        return;
      }

      // Show slash command popover when typing /
      if (e.key === "/" && text === "") {
        setShowSlash(true);
      }

      // Escape closes slash popover
      if (e.key === "Escape" && showSlash) {
        setShowSlash(false);
      }
    },
    [handleSubmit, isBusy, text, showSlash]
  );

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const val = e.target.value;
      setText(val);
      // Show slash commands when text starts with /
      if (val.startsWith("/") && !val.includes(" ")) {
        setShowSlash(true);
      } else {
        setShowSlash(false);
      }
    },
    []
  );

  // Commands that execute immediately without needing args
  const NO_ARG_COMMANDS = new Set([
    "new",
    "compact",
    "undo",
    "redo",
    "share",
    "fork",
    "terminal",
    "clear",
    "models",
    // API commands that take no args
    "gquota",
    "quota",
    "quota_status",
    "tokens_today",
    "tokens_daily",
    "tokens_weekly",
    "tokens_monthly",
    "tokens_all",
    "tokens_session",
  ]);

  const handleSlashSelect = useCallback(
    (command: string) => {
      setShowSlash(false);
      if (NO_ARG_COMMANDS.has(command)) {
        onCommand(command);
        setText("");
      } else {
        setText(`/${command} `);
        textareaRef.current?.focus();
      }
    },
    [onCommand]
  );

  /** Shorten model ID for display */
  function shortModelName(modelId: string): string {
    // Remove common prefixes and keep the meaningful part
    const parts = modelId.split("/");
    const name = parts[parts.length - 1];
    // Truncate to ~30 chars
    return name.length > 30 ? name.slice(0, 28) + "\u2026" : name;
  }

  return (
    <div className="prompt-input-container">
      {/* Slash command popover */}
      {showSlash && (
        <SlashCommandPopover
          filter={text.startsWith("/") ? text.slice(1) : ""}
          onSelect={handleSlashSelect}
          onClose={() => setShowSlash(false)}
          sessionId={sessionId}
        />
      )}

      {/* Model selector bar */}
      <div className="prompt-model-bar">
        <button
          className="prompt-model-btn"
          onClick={onOpenModelPicker}
          title="Change model"
          disabled={disabled}
        >
          <Cpu size={12} />
          <span className="prompt-model-name">
            {currentModel ? shortModelName(currentModel) : "Select model"}
          </span>
          <ChevronDown size={10} />
        </button>
      </div>

      <div className="prompt-input-wrapper">
        <textarea
          ref={textareaRef}
          className="prompt-textarea"
          value={text}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={
            disabled
              ? "Select a session to start..."
              : isBusy
                ? "Waiting for response..."
                : "Type a message... (/ for commands)"
          }
          disabled={disabled}
          rows={1}
        />
        <div className="prompt-actions">
          {isBusy ? (
            <button
              className="prompt-btn prompt-abort-btn"
              onClick={onAbort}
              title="Stop generation"
            >
              <Square size={16} />
            </button>
          ) : isSending ? (
            <button
              className="prompt-btn prompt-send-btn"
              disabled
              title="Sending..."
            >
              <Loader2 size={16} className="spinning" />
            </button>
          ) : (
            <button
              className="prompt-btn prompt-send-btn"
              onClick={handleSubmit}
              disabled={disabled || !text.trim()}
              title="Send (Enter)"
            >
              <Send size={16} />
            </button>
          )}
        </div>
      </div>

      <div className="prompt-hints">
        <span>
          <kbd>Enter</kbd> Send
        </span>
        <span>
          <kbd>Shift+Enter</kbd> Newline
        </span>
        <span>
          <kbd>/</kbd> Commands
        </span>
      </div>
    </div>
  );
}
