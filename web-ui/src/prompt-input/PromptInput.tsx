import React, { useState, useRef, useCallback, useEffect, type KeyboardEvent } from "react";
import type { ImageAttachment } from "../api";
import { SlashCommandPopover } from "../SlashCommandPopover";
import { NO_ARG_COMMANDS } from "./helpers";
import { useAgents, useAttachments, useAtMention } from "./hooks";
import {
  SelectorChips, AgentMentionPills, AttachmentPreviews,
  TextareaRow, DragOverlay, HintBar, AtMentionPopover,
} from "./components";

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
  onOpenMemory?: () => void;
  onContentChange?: (hasContent: boolean) => void;
}

export function PromptInput({
  onSend, onAbort, onCommand, onOpenModelPicker, onOpenAgentPicker,
  isBusy, isSending, disabled, sessionId, currentModel,
  currentAgent, onAgentChange, activeMemoryLabels = [], onOpenMemory, onContentChange,
}: Props) {
  const [text, setText] = useState("");
  const [showSlash, setShowSlash] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const { allAgents, agents, mentionableAgents } = useAgents(currentAgent, onAgentChange);
  const attach = useAttachments();
  const atMention = useAtMention(allAgents, mentionableAgents, textareaRef, text, setText);

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [text]);

  // Focus input on mount and when session changes
  useEffect(() => { textareaRef.current?.focus(); }, [sessionId]);

  // ── Submit handler ───────────────────────────────────
  const handleSubmit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed && attach.attachments.length === 0) return;
    if (trimmed.startsWith("/") && attach.attachments.length === 0) {
      const parts = trimmed.split(/\s+/);
      onCommand(parts[0].slice(1), parts.slice(1).join(" "));
      setText(""); return;
    }
    onSend(trimmed || "Attached image(s)", attach.attachments.length > 0 ? attach.attachments : undefined);
    setText(""); attach.clearAttachments(); atMention.clearMentions();
    onContentChange?.(false);
  }, [text, attach, atMention, onSend, onCommand, onContentChange]);

  const handleKeyDown = useCallback((e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSubmit(); return; }
    if (e.key === "/" && text === "") setShowSlash(true);
    if (e.key === "Escape") {
      if (showSlash) setShowSlash(false);
      if (atMention.showAtPopover) { /* close handled by atMention hook */ }
    }
  }, [handleSubmit, text, showSlash, atMention.showAtPopover]);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setText(val);
    if (val.startsWith("/") && !val.includes(" ")) setShowSlash(true);
    else setShowSlash(false);
    const el = e.target;
    atMention.updateAtState(val, el.selectionStart ?? val.length);
    onContentChange?.(val.trim().length > 0);
  }, [onContentChange, atMention]);

  const handleSlashSelect = useCallback((command: string) => {
    setShowSlash(false);
    if (NO_ARG_COMMANDS.has(command)) {
      onCommand(command); setText(""); onContentChange?.(false);
    } else {
      setText(`/${command} `); onContentChange?.(true); textareaRef.current?.focus();
    }
  }, [onCommand, onContentChange]);

  const hasContent = text.trim().length > 0 || attach.attachments.length > 0;

  return (
    <div className={`prompt-input-container ${attach.dragOver ? "prompt-drag-over" : ""}`}
      onDragEnter={attach.handleDragEnter} onDragLeave={attach.handleDragLeave}
      onDragOver={attach.handleDragOver} onDrop={attach.handleDrop}>
      {attach.dragOver && <DragOverlay />}
      {showSlash && (
        <SlashCommandPopover filter={text.startsWith("/") ? text.slice(1) : ""}
          onSelect={handleSlashSelect} onClose={() => setShowSlash(false)} sessionId={sessionId} />
      )}
      {atMention.showAtPopover && atMention.filteredMentionAgents.length > 0 && (
        <AtMentionPopover agents={atMention.filteredMentionAgents}
          popoverRef={atMention.atPopoverRef} onSelect={atMention.handleAtAgentSelect} />
      )}
      <div className="prompt-input-wrapper">
        <SelectorChips currentModel={currentModel} currentAgent={currentAgent} agents={agents}
          disabled={disabled} activeMemoryLabels={activeMemoryLabels}
          onOpenModelPicker={onOpenModelPicker} onOpenAgentPicker={onOpenAgentPicker} onOpenMemory={onOpenMemory} />
        <AgentMentionPills agentMentions={atMention.agentMentions} allAgents={allAgents}
          onRemove={(id) => atMention.setAgentMentions((prev) => prev.filter((m) => m !== id))} />
        <AttachmentPreviews attachments={attach.attachments} onRemove={attach.removeAttachment} />
        <TextareaRow textareaRef={textareaRef} fileInputRef={attach.fileInputRef}
          text={text} disabled={disabled} isBusy={isBusy} isSending={isSending} hasContent={hasContent}
          onChange={handleChange} onKeyDown={handleKeyDown} onPaste={attach.handlePaste}
          onFileSelect={attach.handleFileSelect} onSubmit={handleSubmit} onAbort={onAbort} />
      </div>
      <HintBar />
    </div>
  );
}
