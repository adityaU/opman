import React from "react";
import {
  Cpu, ChevronDown, Brain, AtSign, X, File, Folder,
  ImageIcon, Paperclip, Send, Square, Loader2, SquareTerminal,
} from "lucide-react";
import type { AgentInfo, ImageAttachment, FileSearchEntry, SessionStats } from "../api";
import type { FileMention } from "./useFileMention";
import { agentColor, shortModelName } from "./helpers";
import { UsageInfoButton } from "./UsagePopover";
import { QueuePill } from "./QueueControls";
export { AtMentionPopover } from "./AtMentionPopover";
import { EngineChip } from "../engine-picker/EngineChip";

// ── SelectorChips ───────────────────────────────────────────────

interface SelectorChipsProps {
  currentModel: string | null;
  /** The exact selection, needed to tell a model apart across providers. */
  selectedModel?: { providerID: string; modelID: string } | null;
  currentAgent: string;
  agents: AgentInfo[];
  disabled: boolean;
  activeMemoryLabels: string[];
  onModelSelected?: (modelId: string, providerId: string) => void;
  onAgentChange?: (agentId: string) => void;
  onOpenMemory?: () => void;
  /** Session token/cost stats — renders an "i" info button with a breakdown when present. */
  stats?: SessionStats | null;
  /** Count of queued follow-up prompts; a pill shows when > 0. */
  queuedCount?: number;
  /** Whether the queue panel is currently open (highlights the pill). */
  queueOpen?: boolean;
  onToggleQueue?: () => void;
  currentRunner?: string;
  availableRunners?: string[];
  onRunnerChange?: (runner: string) => void;
  supportedEfforts?: string[];
  effort?: string | null;
  permission?: string;
  onEffortChange?: (effort: string | null) => void;
  onPermissionChange?: (permission: string) => void;
}

export function SelectorChips({
  currentModel, selectedModel = null, currentAgent, agents, disabled,
  activeMemoryLabels, onModelSelected, onAgentChange, onOpenMemory, stats,
  queuedCount = 0, queueOpen = false, onToggleQueue,
  currentRunner = "opencode", availableRunners = ["opencode", "claude-code", "claude", "codex"], onRunnerChange,
  supportedEfforts = [], effort = null, permission = "default", onEffortChange, onPermissionChange,
}: SelectorChipsProps) {
  return (
    <div className="prompt-selectors">
      <EngineChip
        runner={currentRunner}
        availableRunners={availableRunners}
        currentModel={currentModel}
        selectedModel={selectedModel}
        currentAgent={currentAgent}
        agents={agents}
        disabled={disabled}
        supportedEfforts={supportedEfforts}
        effort={effort}
        permission={permission}
        onRunnerChange={onRunnerChange || (() => {})}
        onModelSelected={onModelSelected || (() => {})}
        onAgentChange={onAgentChange || (() => {})}
        onEffortChange={onEffortChange || (() => {})}
        onPermissionChange={onPermissionChange || (() => {})}
      />
      {activeMemoryLabels.length > 0 && (
        <button className="prompt-chip prompt-chip-memory" onClick={onOpenMemory} title={activeMemoryLabels.join(", ")}>
          <Brain size={11} />
          <span className="prompt-chip-label">
            {activeMemoryLabels.length} {activeMemoryLabels.length === 1 ? "instruction" : "instructions"}
          </span>
        </button>
      )}
      {onToggleQueue && (
        <QueuePill count={queuedCount} active={queueOpen} onClick={onToggleQueue} />
      )}
      <UsageInfoButton stats={stats} />
    </div>
  );
}

// ── AgentMentionPills ───────────────────────────────────────────

interface AgentMentionPillsProps {
  agentMentions: string[];
  allAgents: AgentInfo[];
  onRemove: (id: string) => void;
}

export function AgentMentionPills({ agentMentions, allAgents, onRemove }: AgentMentionPillsProps) {
  if (agentMentions.length === 0) return null;
  return (
    <div className="prompt-agent-mentions">
      {agentMentions.map((id) => {
        const info = allAgents.find((a) => a.id === id);
        const color = agentColor(id, info?.color);
        return (
          <span key={id} className="prompt-agent-pill"
            style={{ borderColor: `color-mix(in srgb, ${color} 27%, transparent)`, backgroundColor: `color-mix(in srgb, ${color} 7%, transparent)` }}>
            <AtSign size={10} />
            <span>{info?.label || id}</span>
            <button className="prompt-agent-pill-remove" onClick={() => onRemove(id)}
              title={`Remove @${info?.label || id}`} aria-label={`Remove @${info?.label || id} mention`}>
              <X size={9} />
            </button>
          </span>
        );
      })}
    </div>
  );
}

// ── FileMentionPills ────────────────────────────────────────────

interface FileMentionPillsProps {
  fileMentions: FileMention[];
  onRemove: (path: string) => void;
}

export function FileMentionPills({ fileMentions, onRemove }: FileMentionPillsProps) {
  if (fileMentions.length === 0) return null;
  return (
    <div className="prompt-file-mentions">
      {fileMentions.map((m) => (
        <span key={m.path} className="prompt-file-pill">
          {m.is_dir ? <Folder size={10} /> : <File size={10} />}
          <span className="prompt-file-pill-path">{m.path}</span>
          <button className="prompt-agent-pill-remove" onClick={() => onRemove(m.path)}
            title={`Remove @${m.path}`} aria-label={`Remove file mention ${m.path}`}>
            <X size={9} />
          </button>
        </span>
      ))}
    </div>
  );
}

// ── AttachmentPreviews ──────────────────────────────────────────

interface AttachmentPreviewsProps {
  attachments: ImageAttachment[];
  onRemove: (index: number) => void;
}

export function AttachmentPreviews({ attachments, onRemove }: AttachmentPreviewsProps) {
  if (attachments.length === 0) return null;
  return (
    <div className="prompt-attachments">
      {attachments.map((att, i) => (
        <div key={i} className="prompt-attachment-thumb">
          <img src={`data:${att.mimeType};base64,${att.base64}`} alt={att.name} className="prompt-attachment-img" />
          <button className="prompt-attachment-remove" onClick={() => onRemove(i)}
            title="Remove attachment" aria-label={`Remove ${att.name}`}>
            <X size={10} />
          </button>
          <span className="prompt-attachment-name">{att.name}</span>
        </div>
      ))}
    </div>
  );
}

// ── TextareaRow ─────────────────────────────────────────────────

interface TextareaRowProps {
  textareaRef: React.RefObject<HTMLTextAreaElement>;
  fileInputRef: React.RefObject<HTMLInputElement>;
  text: string;
  disabled: boolean;
  isBusy: boolean;
  isSending?: boolean;
  hasContent: boolean;
  onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onPaste: (e: React.ClipboardEvent<HTMLTextAreaElement>) => void;
  onFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onSubmit: () => void;
  onAbort: () => void;
  /** Claude engine only: open an interactive terminal attached to this session's agent. */
  onAttachTerminal?: () => void;
}

export function TextareaRow({
  textareaRef, fileInputRef, text, disabled, isBusy, isSending, hasContent,
  onChange, onKeyDown, onPaste, onFileSelect, onSubmit, onAbort, onAttachTerminal,
}: TextareaRowProps) {
  return (
    <div className="prompt-textarea-row">
      <button className="prompt-btn prompt-attach-btn" onClick={() => fileInputRef.current?.click()}
        disabled={disabled} title="Attach image (or paste/drag)" aria-label="Attach image">
        <Paperclip size={15} />
      </button>
      {onAttachTerminal && (
        <button className="prompt-btn prompt-attach-btn prompt-terminal-btn" onClick={onAttachTerminal}
          disabled={disabled} title="Open the claude CLI attached to this session"
          aria-label="Attach terminal">
          <SquareTerminal size={15} />
        </button>
      )}
      <input ref={fileInputRef} type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/bmp"
        multiple onChange={onFileSelect} style={{ display: "none" }} />
      <textarea ref={textareaRef} className="prompt-textarea" value={text}
        onChange={onChange} onKeyDown={onKeyDown} onPaste={onPaste}
        placeholder={disabled ? "Select a session to start..." : isBusy ? "Type a follow-up message..." : "Type a message... (/ for commands, paste or drop images)"}
        disabled={disabled} rows={1} />
      <div className={`prompt-actions${isBusy ? " prompt-actions-busy" : ""}`}>
        {isBusy && (
          <button className="prompt-btn prompt-abort-btn" onClick={onAbort} title="Stop generation" aria-label="Stop generation">
            <Square size={16} />
          </button>
        )}
        {isSending ? (
          <button className="prompt-btn prompt-send-btn" disabled title="Sending..." aria-label="Sending message">
            <Loader2 size={16} className="spinning" />
          </button>
        ) : (
          <button className="prompt-btn prompt-send-btn" onClick={onSubmit}
            disabled={disabled || !hasContent}
            title={isBusy ? "Send follow-up (Enter)" : "Send (Enter)"} aria-label="Send message">
            <Send size={16} />
          </button>
        )}
      </div>
    </div>
  );
}

// ── DragOverlay ─────────────────────────────────────────────────

export function DragOverlay() {
  return (
    <div className="prompt-drag-overlay">
      <ImageIcon size={24} />
      <span>Drop image to attach</span>
    </div>
  );
}

// ── HintBar ─────────────────────────────────────────────────────

export function HintBar() {
  return (
    <div className="prompt-hints">
      <span><kbd>Enter</kbd> Send</span>
      <span><kbd>Shift+Enter</kbd> Newline</span>
      <span><kbd>/</kbd> Commands</span>
      <span><kbd>{navigator.platform.includes("Mac") ? "Cmd" : "Ctrl"}+V</kbd> Paste image</span>
    </div>
  );
}

