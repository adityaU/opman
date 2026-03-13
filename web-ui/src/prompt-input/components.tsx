import React from "react";
import {
  Cpu, ChevronDown, Brain, AtSign, X,
  ImageIcon, Paperclip, Send, Square, Loader2,
} from "lucide-react";
import type { AgentInfo, ImageAttachment } from "../api";
import { agentColor, shortModelName } from "./helpers";

// ── SelectorChips ───────────────────────────────────────────────

interface SelectorChipsProps {
  currentModel: string | null;
  currentAgent: string;
  agents: AgentInfo[];
  disabled: boolean;
  activeMemoryLabels: string[];
  onOpenModelPicker: () => void;
  onOpenAgentPicker: () => void;
  onOpenMemory?: () => void;
}

export function SelectorChips({
  currentModel, currentAgent, agents, disabled,
  activeMemoryLabels, onOpenModelPicker, onOpenAgentPicker, onOpenMemory,
}: SelectorChipsProps) {
  const info = agents.find((a) => a.id === currentAgent);
  const label = info?.label || currentAgent;
  const chipColor = agentColor(currentAgent, info?.color);

  return (
    <div className="prompt-selectors">
      <button className="prompt-chip" onClick={onOpenModelPicker} title="Change model" disabled={disabled}>
        <Cpu size={11} />
        <span className="prompt-chip-label">{currentModel ? shortModelName(currentModel) : "Model"}</span>
        <ChevronDown size={9} />
      </button>
      <button className="prompt-chip" onClick={onOpenAgentPicker} title="Change agent" disabled={disabled}
        style={{ borderColor: `color-mix(in srgb, ${chipColor} 20%, transparent)` }}>
        <span className="prompt-agent-dot" style={{ backgroundColor: chipColor }} />
        <span className="prompt-chip-label">{label}</span>
        <ChevronDown size={9} />
      </button>
      {activeMemoryLabels.length > 0 && (
        <button className="prompt-chip prompt-chip-memory" onClick={onOpenMemory} title={activeMemoryLabels.join(", ")}>
          <Brain size={11} />
          <span className="prompt-chip-label">
            {activeMemoryLabels.length} {activeMemoryLabels.length === 1 ? "memory" : "memories"}
          </span>
        </button>
      )}
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
}

export function TextareaRow({
  textareaRef, fileInputRef, text, disabled, isBusy, isSending, hasContent,
  onChange, onKeyDown, onPaste, onFileSelect, onSubmit, onAbort,
}: TextareaRowProps) {
  return (
    <div className="prompt-textarea-row">
      <button className="prompt-btn prompt-attach-btn" onClick={() => fileInputRef.current?.click()}
        disabled={disabled} title="Attach image (or paste/drag)" aria-label="Attach image">
        <Paperclip size={15} />
      </button>
      <input ref={fileInputRef} type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/bmp"
        multiple onChange={onFileSelect} style={{ display: "none" }} />
      <textarea ref={textareaRef} className="prompt-textarea" value={text}
        onChange={onChange} onKeyDown={onKeyDown} onPaste={onPaste}
        placeholder={disabled ? "Select a session to start..." : isBusy ? "Waiting for response..." : "Type a message... (/ for commands, paste or drop images)"}
        disabled={disabled} rows={1} />
      <div className="prompt-actions">
        {isBusy ? (
          <button className="prompt-btn prompt-abort-btn" onClick={onAbort} title="Stop generation" aria-label="Stop generation">
            <Square size={16} />
          </button>
        ) : isSending ? (
          <button className="prompt-btn prompt-send-btn" disabled title="Sending..." aria-label="Sending message">
            <Loader2 size={16} className="spinning" />
          </button>
        ) : (
          <button className="prompt-btn prompt-send-btn" onClick={onSubmit}
            disabled={disabled || !hasContent} title="Send (Enter)" aria-label="Send message">
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

// ── AtMentionPopover ────────────────────────────────────────────

interface AtMentionPopoverProps {
  agents: AgentInfo[];
  popoverRef: React.RefObject<HTMLDivElement>;
  onSelect: (agentId: string) => void;
}

export function AtMentionPopover({ agents, popoverRef, onSelect }: AtMentionPopoverProps) {
  return (
    <div className="prompt-at-popover" ref={popoverRef}>
      {agents.map((agent) => {
        const color = agentColor(agent.id, agent.color);
        return (
          <button key={agent.id} className="prompt-at-popover-item" onClick={() => onSelect(agent.id)}>
            {color ? <span className="prompt-agent-dot" style={{ backgroundColor: color }} /> : <AtSign size={12} />}
            <span className="prompt-at-popover-name">{agent.label}</span>
            <span className="prompt-at-popover-desc">{agent.description}</span>
          </button>
        );
      })}
    </div>
  );
}
