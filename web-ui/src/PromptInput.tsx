import React, {
  useState,
  useRef,
  useCallback,
  useEffect,
  type KeyboardEvent,
  type DragEvent,
  type ClipboardEvent,
} from "react";
import { Send, Square, Cpu, ChevronDown, Loader2, Bot, Paperclip, X, Image as ImageIcon, Brain, AtSign } from "lucide-react";
import { SlashCommandPopover } from "./SlashCommandPopover";
import { fetchAgents, type AgentInfo, type ImageAttachment } from "./api";

/** Default agent colours keyed by id (mirrors opencode's agentColor utility) */
const AGENT_COLORS: Record<string, string> = {
  coder: "#3b82f6",   // blue
  task: "#f59e0b",    // amber
  ask: "#8b5cf6",     // purple
  build: "#10b981",   // emerald
  docs: "#06b6d4",    // cyan
  plan: "#f43f5e",    // rose
};

/** Resolve the display colour for an agent */
function agentColor(id: string, custom?: string): string | undefined {
  if (custom) return custom;
  return AGENT_COLORS[id] ?? AGENT_COLORS[id.toLowerCase()];
}

/** Fallback agents if fetch fails or is pending */
const DEFAULT_AGENTS: AgentInfo[] = [
  { id: "build", label: "Build", description: "Default coding agent", mode: "primary", native: true },
  { id: "plan", label: "Plan", description: "Planning and design agent", mode: "all", native: true },
];

/**
 * Filter agents the same way opencode does: hide agents with mode "subagent"
 * and those explicitly marked hidden.
 */
function selectableAgents(agents: AgentInfo[]): AgentInfo[] {
  return agents.filter((a) => a.mode !== "subagent" && !a.hidden);
}

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
  onOpenAgentPicker: () => void;
  isBusy: boolean;
  isSending?: boolean;
  disabled: boolean;
  sessionId: string | null;
  currentModel: string | null;
  currentAgent: string;
  onAgentChange: (agent: string) => void;
  activeMemoryLabels?: string[];
  /** Open the memory modal */
  onOpenMemory?: () => void;
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
  onOpenAgentPicker,
  isBusy,
  isSending,
  disabled,
  sessionId,
  currentModel,
  currentAgent,
  onAgentChange,
  activeMemoryLabels = [],
  onOpenMemory,
  onContentChange,
}: Props) {
  const [text, setText] = useState("");
  const [showSlash, setShowSlash] = useState(false);
  const [allAgents, setAllAgents] = useState<AgentInfo[]>(DEFAULT_AGENTS);
  const [agentMentions, setAgentMentions] = useState<string[]>([]);
  const [showAtPopover, setShowAtPopover] = useState(false);
  const [atFilter, setAtFilter] = useState("");
  const [attachments, setAttachments] = useState<ImageAttachment[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const atPopoverRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const dragCountRef = useRef(0);

  // Derive the selectable agents (filter out subagent/hidden, like opencode)
  const agents = selectableAgents(allAgents);
  // All non-primary agents available for @mention (subagents excluded from selector
  // but all non-hidden agents with mode !== "primary" can be mentioned inline)
  const mentionableAgents = allAgents.filter((a) => !a.hidden && a.mode !== "primary");

  // Fetch agents from server on mount
  useEffect(() => {
    fetchAgents().then((fetched) => {
      if (fetched.length > 0) {
        setAllAgents(fetched);
        // Auto-select the first selectable agent if none is currently selected
        const selectable = selectableAgents(fetched);
        if (selectable.length > 0 && (!currentAgent || !selectable.some((a) => a.id === currentAgent))) {
          onAgentChange(selectable[0].id);
        }
      }
    });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

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

  // Close @agent popover on outside click
  useEffect(() => {
    if (!showAtPopover) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (atPopoverRef.current && !atPopoverRef.current.contains(e.target as Node)) {
        setShowAtPopover(false);
        setAtFilter("");
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showAtPopover]);

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
    setAgentMentions([]);
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

      // Escape closes slash popover or @mention popover
      if (e.key === "Escape") {
        if (showSlash) setShowSlash(false);
        if (showAtPopover) { setShowAtPopover(false); setAtFilter(""); }
      }
    },
    [handleSubmit, text, showSlash, showAtPopover]
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
      // Detect @agent mention trigger: look for @ followed by word chars at the cursor
      const el = e.target;
      const pos = el.selectionStart ?? val.length;
      const before = val.slice(0, pos);
      const atMatch = before.match(/@(\w*)$/);
      if (atMatch && mentionableAgents.length > 0) {
        setShowAtPopover(true);
        setAtFilter(atMatch[1].toLowerCase());
      } else {
        setShowAtPopover(false);
        setAtFilter("");
      }
      // Notify parent about content state (for mobile input autohide guard)
      onContentChange?.(val.trim().length > 0);
    },
    [onContentChange, mentionableAgents]
  );

  /** Handle selecting an agent from the @mention popover */
  const handleAtAgentSelect = useCallback(
    (agentId: string) => {
      setShowAtPopover(false);
      setAtFilter("");
      // Replace the @partial text in the textarea with the full @agent mention
      const el = textareaRef.current;
      if (!el) return;
      const pos = el.selectionStart ?? text.length;
      const before = text.slice(0, pos);
      const after = text.slice(pos);
      const atIdx = before.lastIndexOf("@");
      if (atIdx === -1) return;
      const newText = before.slice(0, atIdx) + after;
      setText(newText);
      // Add to mentions if not already there
      if (!agentMentions.includes(agentId)) {
        setAgentMentions((prev) => [...prev, agentId]);
      }
      // Re-focus textarea
      setTimeout(() => {
        const t = textareaRef.current;
        if (t) {
          t.focus();
          const newPos = atIdx;
          t.setSelectionRange(newPos, newPos);
        }
      }, 0);
    },
    [text, agentMentions]
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

  /** Shorten model ID for display */
  function shortModelName(modelId: string): string {
    const parts = modelId.split("/");
    const name = parts[parts.length - 1];
    return name.length > 30 ? name.slice(0, 28) + "\u2026" : name;
  }

  const currentAgentInfo = agents.find((a) => a.id === currentAgent);
  const agentLabel = currentAgentInfo?.label || currentAgent;
  const chipColor = agentColor(currentAgent, currentAgentInfo?.color);

  // Filter mentionable agents by the @filter text
  const filteredMentionAgents = mentionableAgents.filter(
    (a) => atFilter === "" || a.id.toLowerCase().includes(atFilter) || a.label.toLowerCase().includes(atFilter)
  );

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

      {/* @agent mention popover — positioned above input like slash commands */}
      {showAtPopover && filteredMentionAgents.length > 0 && (
        <div className="prompt-at-popover" ref={atPopoverRef}>
          {filteredMentionAgents.map((agent) => {
            const color = agentColor(agent.id, agent.color);
            return (
              <button
                key={agent.id}
                className="prompt-at-popover-item"
                onClick={() => handleAtAgentSelect(agent.id)}
              >
                {color ? (
                  <span className="prompt-agent-dot" style={{ backgroundColor: color }} />
                ) : (
                  <AtSign size={12} />
                )}
                <span className="prompt-at-popover-name">{agent.label}</span>
                <span className="prompt-at-popover-desc">{agent.description}</span>
              </button>
            );
          })}
        </div>
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

          {/* Agent chip (colour-coded like opencode) — opens modal */}
          <button
            className="prompt-chip"
            onClick={onOpenAgentPicker}
            title="Change agent"
            disabled={disabled}
            style={chipColor ? { borderColor: `${chipColor}33` } : undefined}
          >
            {chipColor ? (
              <span
                className="prompt-agent-dot"
                style={{ backgroundColor: chipColor }}
              />
            ) : (
              <Bot size={11} />
            )}
            <span className="prompt-chip-label">{agentLabel}</span>
            <ChevronDown size={9} />
          </button>

          {/* Memory chip (compact count in selector row) */}
          {activeMemoryLabels.length > 0 && (
            <button
              className="prompt-chip prompt-chip-memory"
              onClick={onOpenMemory}
              title={activeMemoryLabels.join(", ")}
            >
              <Brain size={11} />
              <span className="prompt-chip-label">
                {activeMemoryLabels.length} {activeMemoryLabels.length === 1 ? "memory" : "memories"}
              </span>
            </button>
          )}
        </div>

        {/* @agent mention pills */}
        {agentMentions.length > 0 && (
          <div className="prompt-agent-mentions">
            {agentMentions.map((id) => {
              const info = allAgents.find((a) => a.id === id);
              const color = agentColor(id, info?.color);
              return (
                <span
                  key={id}
                  className="prompt-agent-pill"
                  style={color ? { borderColor: `${color}44`, backgroundColor: `${color}11` } : undefined}
                >
                  <AtSign size={10} />
                  <span>{info?.label || id}</span>
                  <button
                    className="prompt-agent-pill-remove"
                    onClick={() => setAgentMentions((prev) => prev.filter((m) => m !== id))}
                    title={`Remove @${info?.label || id}`}
                    aria-label={`Remove @${info?.label || id} mention`}
                  >
                    <X size={9} />
                  </button>
                </span>
              );
            })}
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
