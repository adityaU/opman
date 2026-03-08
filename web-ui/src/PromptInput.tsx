import React, {
  useState,
  useRef,
  useCallback,
  useEffect,
  type KeyboardEvent,
  type DragEvent,
  type ClipboardEvent,
} from "react";
import { Send, Square, Cpu, ChevronDown, Loader2, Bot, Paperclip, X, Image as ImageIcon } from "lucide-react";
import { SlashCommandPopover } from "./SlashCommandPopover";
import { fetchAgents, type AgentInfo, type ImageAttachment } from "./api";

/** Fallback agents if fetch fails or is pending */
const DEFAULT_AGENTS: AgentInfo[] = [
  { id: "coder", label: "Coder", description: "Default coding agent" },
  { id: "task", label: "Task", description: "Autonomous task agent" },
];

/** Max file size for image attachments (10 MB) */
const MAX_IMAGE_SIZE = 10 * 1024 * 1024;

/** Accepted image MIME types */
const ACCEPTED_IMAGE_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/svg+xml",
  "image/bmp",
]);

interface Props {
  onSend: (text: string, images?: ImageAttachment[]) => void;
  onAbort: () => void;
  onCommand: (command: string, args?: string) => void;
  onOpenModelPicker: () => void;
  isBusy: boolean;
  isSending?: boolean;
  disabled: boolean;
  sessionId: string | null;
  currentModel: string | null;
  currentAgent: string;
  onAgentChange: (agent: string) => void;
  activeMemoryLabels?: string[];
  /** Reports whether the textarea has non-empty content (for mobile input autohide guard) */
  onContentChange?: (hasContent: boolean) => void;
}

/** Convert a File to an ImageAttachment via base64 */
function fileToImageAttachment(file: File): Promise<ImageAttachment> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      // Strip "data:image/png;base64," prefix
      const base64 = dataUrl.split(",")[1] || "";
      resolve({ base64, mimeType: file.type, name: file.name || "pasted-image" });
    };
    reader.onerror = () => reject(new Error("Failed to read file"));
    reader.readAsDataURL(file);
  });
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
  currentAgent,
  onAgentChange,
  activeMemoryLabels = [],
  onContentChange,
}: Props) {
  const [text, setText] = useState("");
  const [showSlash, setShowSlash] = useState(false);
  const [showAgentDropdown, setShowAgentDropdown] = useState(false);
  const [agents, setAgents] = useState<AgentInfo[]>(DEFAULT_AGENTS);
  const [attachments, setAttachments] = useState<ImageAttachment[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const agentDropdownRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const dragCountRef = useRef(0);

  // Fetch agents from server on mount
  useEffect(() => {
    fetchAgents().then((fetched) => {
      if (fetched.length > 0) setAgents(fetched);
    });
  }, []);

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

  // Close agent dropdown on outside click
  useEffect(() => {
    if (!showAgentDropdown) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (agentDropdownRef.current && !agentDropdownRef.current.contains(e.target as Node)) {
        setShowAgentDropdown(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showAgentDropdown]);

  // ── Attachment helpers ────────────────────────────────

  const addImageFiles = useCallback(async (files: FileList | File[]) => {
    const imageFiles = Array.from(files).filter(
      (f) => ACCEPTED_IMAGE_TYPES.has(f.type) && f.size <= MAX_IMAGE_SIZE
    );
    if (imageFiles.length === 0) return;
    try {
      const newAttachments = await Promise.all(imageFiles.map(fileToImageAttachment));
      setAttachments((prev) => [...prev, ...newAttachments]);
    } catch {
      // silently ignore read errors
    }
  }, []);

  const removeAttachment = useCallback((index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index));
  }, []);

  // ── Paste handler ────────────────────────────────────

  const handlePaste = useCallback(
    (e: ClipboardEvent<HTMLTextAreaElement>) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      const imageFiles: File[] = [];
      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.kind === "file" && ACCEPTED_IMAGE_TYPES.has(item.type)) {
          const file = item.getAsFile();
          if (file) imageFiles.push(file);
        }
      }
      if (imageFiles.length > 0) {
        e.preventDefault();
        addImageFiles(imageFiles);
      }
    },
    [addImageFiles]
  );

  // ── Drag-and-drop handlers ───────────────────────────

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCountRef.current++;
    if (e.dataTransfer?.types?.includes("Files")) {
      setDragOver(true);
    }
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCountRef.current--;
    if (dragCountRef.current <= 0) {
      dragCountRef.current = 0;
      setDragOver(false);
    }
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      dragCountRef.current = 0;
      setDragOver(false);
      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        addImageFiles(files);
      }
    },
    [addImageFiles]
  );

  // ── File input handler ───────────────────────────────

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (files && files.length > 0) {
        addImageFiles(files);
      }
      // Reset so the same file can be selected again
      e.target.value = "";
    },
    [addImageFiles]
  );

  // ── Submit handler ───────────────────────────────────

  const handleSubmit = useCallback(() => {
    const trimmed = text.trim();
    // Allow sending with only attachments (no text)
    if (!trimmed && attachments.length === 0) return;

    // Check for slash commands (only when no attachments)
    if (trimmed.startsWith("/") && attachments.length === 0) {
      const parts = trimmed.split(/\s+/);
      const cmd = parts[0].slice(1); // Remove leading /
      const args = parts.slice(1).join(" ");
      onCommand(cmd, args);
      setText("");
      return;
    }

    onSend(trimmed || "Attached image(s)", attachments.length > 0 ? attachments : undefined);
    setText("");
    setAttachments([]);
    onContentChange?.(false);
  }, [text, attachments, onSend, onCommand, onContentChange]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter sends, Shift+Enter for newline
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
        return;
      }

      // Show slash command popover when typing /
      if (e.key === "/" && text === "") {
        setShowSlash(true);
      }

      // Escape closes slash popover or agent dropdown
      if (e.key === "Escape") {
        if (showSlash) setShowSlash(false);
        if (showAgentDropdown) setShowAgentDropdown(false);
      }
    },
    [handleSubmit, text, showSlash, showAgentDropdown]
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
      // Notify parent about content state (for mobile input autohide guard)
      onContentChange?.(val.trim().length > 0);
    },
    [onContentChange]
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
    // Modal commands
    "keys",
    "keybindings",
    "todos",
    "sessions",
    "context",
    "settings",
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
        onContentChange?.(false);
      } else {
        setText(`/${command} `);
        onContentChange?.(true);
        textareaRef.current?.focus();
      }
    },
    [onCommand, onContentChange]
  );

  const handleAgentSelect = useCallback(
    (agentId: string) => {
      setShowAgentDropdown(false);
      onAgentChange(agentId);
      textareaRef.current?.focus();
    },
    [onAgentChange]
  );

  /** Shorten model ID for display */
  function shortModelName(modelId: string): string {
    const parts = modelId.split("/");
    const name = parts[parts.length - 1];
    return name.length > 30 ? name.slice(0, 28) + "\u2026" : name;
  }

  const agentLabel = agents.find((a) => a.id === currentAgent)?.label || currentAgent;

  return (
    <div
      className={`prompt-input-container ${dragOver ? "prompt-drag-over" : ""}`}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {/* Drag overlay */}
      {dragOver && (
        <div className="prompt-drag-overlay">
          <ImageIcon size={24} />
          <span>Drop image to attach</span>
        </div>
      )}

      {/* Slash command popover */}
      {showSlash && (
        <SlashCommandPopover
          filter={text.startsWith("/") ? text.slice(1) : ""}
          onSelect={handleSlashSelect}
          onClose={() => setShowSlash(false)}
          sessionId={sessionId}
        />
      )}

      <div className="prompt-input-wrapper">
        {/* Inline selector chips row */}
        <div className="prompt-selectors">
          {/* Model chip */}
          <button
            className="prompt-chip"
            onClick={onOpenModelPicker}
            title="Change model"
            disabled={disabled}
          >
            <Cpu size={11} />
            <span className="prompt-chip-label">
              {currentModel ? shortModelName(currentModel) : "Model"}
            </span>
            <ChevronDown size={9} />
          </button>

          {/* Agent chip */}
          <div className="prompt-chip-container" ref={agentDropdownRef}>
            <button
              className="prompt-chip"
              onClick={() => setShowAgentDropdown((v) => !v)}
              title="Change agent"
              disabled={disabled}
            >
              <Bot size={11} />
              <span className="prompt-chip-label">{agentLabel}</span>
              <ChevronDown size={9} />
            </button>

            {/* Agent dropdown */}
            {showAgentDropdown && (
              <div className="prompt-chip-dropdown">
                {agents.map((agent) => (
                  <button
                    key={agent.id}
                    className={`prompt-chip-dropdown-item ${agent.id === currentAgent ? "active" : ""}`}
                    onClick={() => handleAgentSelect(agent.id)}
                  >
                    <span className="prompt-chip-dropdown-name">{agent.label}</span>
                    <span className="prompt-chip-dropdown-desc">{agent.description}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {activeMemoryLabels.length > 0 && (
          <div className="prompt-memory-strip">
            <span className="prompt-memory-strip-label">Memory applied</span>
            {activeMemoryLabels.slice(0, 4).map((label) => (
              <span key={label} className="prompt-memory-chip">{label}</span>
            ))}
          </div>
        )}

        {/* Attachment previews */}
        {attachments.length > 0 && (
          <div className="prompt-attachments">
            {attachments.map((att, i) => (
              <div key={i} className="prompt-attachment-thumb">
                <img
                  src={`data:${att.mimeType};base64,${att.base64}`}
                  alt={att.name}
                  className="prompt-attachment-img"
                />
                <button
                  className="prompt-attachment-remove"
                  onClick={() => removeAttachment(i)}
                  title="Remove attachment"
                  aria-label={`Remove ${att.name}`}
                >
                  <X size={10} />
                </button>
                <span className="prompt-attachment-name">{att.name}</span>
              </div>
            ))}
          </div>
        )}

        {/* Textarea row */}
        <div className="prompt-textarea-row">
          {/* Attach button */}
          <button
            className="prompt-btn prompt-attach-btn"
            onClick={() => fileInputRef.current?.click()}
            disabled={disabled}
            title="Attach image (or paste/drag)"
            aria-label="Attach image"
          >
            <Paperclip size={15} />
          </button>
          <input
            ref={fileInputRef}
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/bmp"
            multiple
            onChange={handleFileSelect}
            style={{ display: "none" }}
          />

          <textarea
            ref={textareaRef}
            className="prompt-textarea"
            value={text}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            placeholder={
              disabled
                ? "Select a session to start..."
                : isBusy
                  ? "Waiting for response..."
                  : "Type a message... (/ for commands, paste or drop images)"
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
                aria-label="Stop generation"
              >
                <Square size={16} />
              </button>
            ) : isSending ? (
              <button
                className="prompt-btn prompt-send-btn"
                disabled
                title="Sending..."
                aria-label="Sending message"
              >
                <Loader2 size={16} className="spinning" />
              </button>
            ) : (
              <button
                className="prompt-btn prompt-send-btn"
                onClick={handleSubmit}
                disabled={disabled || (!text.trim() && attachments.length === 0)}
                title="Send (Enter)"
                aria-label="Send message"
              >
                <Send size={16} />
              </button>
            )}
          </div>
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
        <span>
          <kbd>{navigator.platform.includes("Mac") ? "Cmd" : "Ctrl"}+V</kbd> Paste image
        </span>
      </div>
    </div>
  );
}
