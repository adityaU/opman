import React, { useState, useRef, useCallback, useEffect, type KeyboardEvent } from "react";
import type { ImageAttachment, FileSearchEntry } from "../api";
import { SlashCommandPopover } from "../SlashCommandPopover";
import { NO_ARG_COMMANDS } from "./helpers";
import { useAgents, useAttachments, useAtMention } from "./hooks";
import { useFileMention } from "./useFileMention";
import { useVoiceDictation } from "../voice/useVoiceDictation";
import {
  SelectorChips, AgentMentionPills, FileMentionPills, AttachmentPreviews,
  TextareaRow, DragOverlay, HintBar, AtMentionPopover,
} from "./components";

interface Props {
  onSend: (text: string, images?: ImageAttachment[], fileContext?: string) => Promise<boolean>;
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
  const fileMention = useFileMention();
  const submittingRef = useRef(false);

  // Voice dictation (desktop only) — appends transcript to current text
  const handleTranscript = useCallback((transcript: string) => {
    setText(prev => {
      const next = prev ? `${prev} ${transcript}` : transcript;
      onContentChange?.(next.trim().length > 0);
      return next;
    });
    textareaRef.current?.focus();
  }, [onContentChange]);
  const voice = useVoiceDictation(handleTranscript);

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [text]);

  // Focus input on mount and when session changes (desktop only)
  useEffect(() => {
    if (window.innerWidth >= 768) textareaRef.current?.focus();
  }, [sessionId]);

  // Clear file mentions on session change
  useEffect(() => { fileMention.clearFileMentions(); }, [sessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Trigger file search when @ filter changes
  useEffect(() => {
    if (atMention.showAtPopover && atMention.atFilter !== undefined) {
      fileMention.searchFilesDebounced(atMention.atFilter);
    } else {
      fileMention.clearFileResults();
    }
  }, [atMention.showAtPopover, atMention.atFilter]); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Submit handler ───────────────────────────────────
  const handleSubmit = useCallback(async () => {
    const trimmed = text.trim();
    if (!trimmed && attach.attachments.length === 0 && fileMention.fileMentions.length === 0) return;
    if (trimmed.startsWith("/") && attach.attachments.length === 0) {
      const parts = trimmed.split(/\s+/);
      onCommand(parts[0].slice(1), parts.slice(1).join(" "));
      setText(""); onContentChange?.(false); return;
    }
    const images = attach.attachments.length > 0 ? [...attach.attachments] : undefined;
    const mentions = [...fileMention.fileMentions];
    // Clear input immediately (optimistic)
    setText(""); attach.clearAttachments(); atMention.clearMentions(); fileMention.clearFileMentions();
    onContentChange?.(false);
    const fileCtx = mentions.length > 0 ? await fileMention.buildFileContextFrom(mentions) : undefined;
    await onSend(trimmed || "Attached image(s)", images, fileCtx || undefined);
  }, [text, attach, atMention, fileMention, onSend, onCommand, onContentChange]);

  const handleKeyDown = useCallback((e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
      return;
    }
    if (e.key === "/" && text === "") setShowSlash(true);
    if (e.key === "Escape") {
      if (showSlash) setShowSlash(false);
    }
  }, [handleSubmit, text, showSlash]);

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

  const handleFileSelect = useCallback((entry: FileSearchEntry) => {
    fileMention.addFileMention(entry);
    // Remove the @query text from the input (same pattern as agent select)
    const el = textareaRef.current;
    if (el) {
      const pos = el.selectionStart ?? text.length;
      const before = text.slice(0, pos);
      const after = text.slice(pos);
      const atIdx = before.lastIndexOf("@");
      if (atIdx !== -1) {
        const newText = before.slice(0, atIdx) + after;
        setText(newText);
        setTimeout(() => { el.focus(); el.setSelectionRange(atIdx, atIdx); }, 0);
      }
    }
    atMention.closePopover();
    fileMention.clearFileResults();
  }, [fileMention, atMention, text]);

  const showPopover = atMention.showAtPopover && (
    atMention.filteredMentionAgents.length > 0 ||
    fileMention.fileResults.length > 0 ||
    fileMention.fileLoading
  );
  const hasContent = text.trim().length > 0 || attach.attachments.length > 0 || fileMention.fileMentions.length > 0;

  return (
    <div className={`prompt-input-container ${attach.dragOver ? "prompt-drag-over" : ""}`}
      onDragEnter={attach.handleDragEnter} onDragLeave={attach.handleDragLeave}
      onDragOver={attach.handleDragOver} onDrop={attach.handleDrop}>
      {attach.dragOver && <DragOverlay />}
      {showSlash && (
        <SlashCommandPopover filter={text.startsWith("/") ? text.slice(1) : ""}
          onSelect={handleSlashSelect} onClose={() => setShowSlash(false)} sessionId={sessionId} />
      )}
      {showPopover && (
        <AtMentionPopover agents={atMention.filteredMentionAgents}
          fileResults={fileMention.fileResults} fileLoading={fileMention.fileLoading}
          popoverRef={atMention.atPopoverRef}
          onSelectAgent={atMention.handleAtAgentSelect} onSelectFile={handleFileSelect} />
      )}
      <div className="prompt-input-wrapper">
        <SelectorChips currentModel={currentModel} currentAgent={currentAgent} agents={agents}
          disabled={disabled} activeMemoryLabels={activeMemoryLabels}
          onOpenModelPicker={onOpenModelPicker} onOpenAgentPicker={onOpenAgentPicker} onOpenMemory={onOpenMemory} />
        <AgentMentionPills agentMentions={atMention.agentMentions} allAgents={allAgents}
          onRemove={(id) => atMention.setAgentMentions((prev) => prev.filter((m) => m !== id))} />
        <FileMentionPills fileMentions={fileMention.fileMentions} onRemove={fileMention.removeFileMention} />
        <AttachmentPreviews attachments={attach.attachments} onRemove={attach.removeAttachment} />
        <TextareaRow textareaRef={textareaRef} fileInputRef={attach.fileInputRef}
          text={text} disabled={disabled} isBusy={isBusy} isSending={isSending} hasContent={hasContent}
          onChange={handleChange} onKeyDown={handleKeyDown} onPaste={attach.handlePaste}
          onFileSelect={attach.handleFileSelect} onSubmit={handleSubmit} onAbort={onAbort}
          voiceStatus={voice.isDesktop ? voice.status : undefined}
          voiceDownloadPercent={voice.isDesktop ? voice.downloadPercent : undefined}
          voiceWaveform={voice.isDesktop ? voice.waveform : undefined}
          voiceError={voice.isDesktop ? voice.error : undefined}
          onVoiceToggle={voice.isDesktop ? voice.toggle : undefined} />
      </div>
      <HintBar />
    </div>
  );
}
