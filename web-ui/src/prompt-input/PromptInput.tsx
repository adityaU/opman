import { isMobileViewport } from "../hooks/useIsMobile";
import React, { useState, useRef, useCallback, useEffect, type KeyboardEvent } from "react";
import type { ImageAttachment, FileSearchEntry, SessionStats } from "../api";
import type { SlashCommand } from "../types";
import { SlashCommandPopover } from "../SlashCommandPopover";
import { takesArguments } from "./helpers";
import { useAgents, useAttachments, useAtMention } from "./hooks";
import { useFileMention } from "./useFileMention";
import { useMessageQueue } from "./useMessageQueue";
import { QueuePanel } from "./QueueControls";
import {
  SelectorChips, AgentMentionPills, FileMentionPills, AttachmentPreviews,
  DragOverlay, AtMentionPopover,
} from "./components";
import { ComposerField, ComposerActions } from "./ComposerBar";
import { SessionLine } from "./SessionLine";

interface Props {
  onSend: (text: string, images?: ImageAttachment[], fileContext?: string) => Promise<boolean>;
  onAbort: () => void;
  onCommand: (command: string, args?: string) => void;
  /** Kept for the command palette's "/models" and "/agent" entry points. */
  onOpenModelPicker: () => void;
  onOpenAgentPicker: () => void;
  /** Direct selection from the composer's engine chip. */
  selectedModel?: { providerID: string; modelID: string } | null;
  onModelSelected?: (modelId: string, providerId: string) => void;
  isBusy: boolean;
  isSending?: boolean;
  disabled: boolean;
  sessionId: string | null;
  currentModel: string | null;
  currentAgent: string;
  onAgentChange: (agent: string) => void;
  currentRunner?: string;
  availableRunners?: string[];
  onRunnerChange?: (runner: string) => void;
  supportedEfforts?: string[];
  effort?: string | null;
  permission?: string;
  onEffortChange?: (effort: string | null) => void;
  onPermissionChange?: (permission: string) => void;
  activeMemoryLabels?: string[];
  onOpenMemory?: () => void;
  onContentChange?: (hasContent: boolean) => void;
  /** Active engine backend ("opencode" | "claude-code"). */
  backend?: string;
  /** Open an interactive terminal attached to this session's claude CLI agent. */
  /** Session token/cost stats, shown via the pill row's usage info button. */
  stats?: SessionStats | null;
  /** Latest live runner progress title, shown on the composer's session line. */
  progressText?: string | null;
  /** Name of the session this composer writes to; editable in place. */
  sessionTitle?: string | null;
}

export function PromptInput({
  onSend, onAbort, onCommand, onOpenModelPicker, onOpenAgentPicker,
  selectedModel = null, onModelSelected,
  isBusy, isSending, disabled, sessionId, currentModel,
  currentAgent, onAgentChange, activeMemoryLabels = [], onOpenMemory, onContentChange,
  backend, stats, currentRunner = "opencode", availableRunners = ["opencode", "claude-code", "claude", "codex"], onRunnerChange,
  supportedEfforts = [], effort = null, permission = "default", onEffortChange, onPermissionChange,
  progressText = null, sessionTitle = null,
}: Props) {
  const [text, setText] = useState("");
  const [showSlash, setShowSlash] = useState(false);
  const [showQueue, setShowQueue] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const queue = useMessageQueue(sessionId);

  // Auto-close the queue panel once the queue empties (e.g. flushed on the next turn).
  useEffect(() => {
    if (queue.queued.length === 0) setShowQueue(false);
  }, [queue.queued.length]);

  const { allAgents, agents, mentionableAgents } = useAgents(currentAgent, onAgentChange, currentRunner);
  const attach = useAttachments();
  const atMention = useAtMention(allAgents, mentionableAgents, textareaRef, text, setText);
  const fileMention = useFileMention();
  const submittingRef = useRef(false);

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [text]);

  // Focus input on mount and when session changes (desktop only)
  useEffect(() => {
    if (!isMobileViewport()) textareaRef.current?.focus();
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
    if (submittingRef.current) return;
    const trimmed = text.trim();
    if (!trimmed && attach.attachments.length === 0 && fileMention.fileMentions.length === 0) return;
    if (trimmed.startsWith("/") && attach.attachments.length === 0) {
      const parts = trimmed.split(/\s+/);
      onCommand(parts[0].slice(1), parts.slice(1).join(" "));
      setText(""); onContentChange?.(false); return;
    }
    const images = attach.attachments.length > 0 ? [...attach.attachments] : undefined;
    const mentions = [...fileMention.fileMentions];
    submittingRef.current = true;
    // Clear input immediately (optimistic)
    setText(""); attach.clearAttachments(); atMention.clearMentions(); fileMention.clearFileMentions();
    onContentChange?.(false);
    try {
      const fileCtx = mentions.length > 0 ? await fileMention.buildFileContextFrom(mentions) : undefined;
      await onSend(trimmed || "Attached image(s)", images, fileCtx || undefined);
    } finally {
      submittingRef.current = false;
    }
  }, [text, attach, atMention, fileMention, onSend, onCommand, onContentChange]);

  const handleKeyDown = useCallback((e: KeyboardEvent<HTMLTextAreaElement>) => {
    // The slash list owns Enter while it is open — it is picking a command, not
    // sending the half-typed name that filtered it.
    if (e.key === "Enter" && !e.shiftKey && !showSlash) {
      e.preventDefault();
      handleSubmit();
      return;
    }
    if (e.key === "/" && text === "") setShowSlash(true);
    // Escape closes whatever the composer has open, and once nothing is open it
    // drops focus so the shell keys work again.
    if (e.key === "Escape") {
      if (showSlash) { setShowSlash(false); return; }
      if (atMention.showAtPopover) return; // the popover's own handler owns it
      e.preventDefault();
      e.currentTarget.blur();
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

  const handleSlashSelect = useCallback((command: SlashCommand) => {
    setShowSlash(false);
    if (takesArguments(command)) {
      setText(`/${command.name} `); onContentChange?.(true); textareaRef.current?.focus();
      return;
    }
    onCommand(command.name); setText(""); onContentChange?.(false);
  }, [onCommand, onContentChange]);

  const handleSlashClick = useCallback(() => {
    if (!text.startsWith("/")) {
      setText("/");
      onContentChange?.(true);
    }
    setShowSlash(true);
    textareaRef.current?.focus();
  }, [text, onContentChange]);

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
          onSelect={handleSlashSelect} onClose={() => setShowSlash(false)} sessionId={sessionId} runner={currentRunner} />
      )}
      {showPopover && (
        <AtMentionPopover agents={atMention.filteredMentionAgents}
          fileResults={fileMention.fileResults} fileLoading={fileMention.fileLoading}
          popoverRef={atMention.atPopoverRef}
          onSelectAgent={atMention.handleAtAgentSelect} onSelectFile={handleFileSelect}
          onClose={atMention.closePopover} />
      )}
      {showQueue && (
        <QueuePanel queued={queue.queued} onRemove={queue.removeAt}
          onClear={queue.clearAll} onClose={() => setShowQueue(false)} />
      )}
      <div className="prompt-input-wrapper">
        <SessionLine title={sessionTitle} sessionId={sessionId} busy={isBusy} progressText={progressText} />
        <AgentMentionPills agentMentions={atMention.agentMentions} allAgents={allAgents}
          onRemove={(id) => atMention.setAgentMentions((prev) => prev.filter((m) => m !== id))} />
        <FileMentionPills fileMentions={fileMention.fileMentions} onRemove={fileMention.removeFileMention} />
        <AttachmentPreviews attachments={attach.attachments} onRemove={attach.removeAttachment} />
        <ComposerField textareaRef={textareaRef} fileInputRef={attach.fileInputRef}
          text={text} disabled={disabled} isBusy={isBusy} runner={currentRunner}
          onChange={handleChange} onKeyDown={handleKeyDown} onPaste={attach.handlePaste}
          onFileSelect={attach.handleFileSelect} />
        <div className="composer-bar">
          <SelectorChips currentModel={currentModel} currentAgent={currentAgent} agents={agents}
             currentRunner={currentRunner} availableRunners={availableRunners} onRunnerChange={onRunnerChange}
             supportedEfforts={supportedEfforts} effort={effort} permission={permission}
             onEffortChange={onEffortChange} onPermissionChange={onPermissionChange}
            disabled={disabled} activeMemoryLabels={activeMemoryLabels} stats={stats}
            selectedModel={selectedModel} onModelSelected={onModelSelected} onAgentChange={onAgentChange}
            onOpenMemory={onOpenMemory}
            queuedCount={queue.queued.length} queueOpen={showQueue}
            onToggleQueue={() => setShowQueue((v) => !v)} />
          <ComposerActions disabled={disabled} isBusy={isBusy} isSending={isSending}
            hasContent={hasContent}
            onAttachClick={() => attach.fileInputRef.current?.click()}
            onSlashClick={handleSlashClick}
            onSubmit={handleSubmit} onAbort={onAbort} />
        </div>
      </div>
    </div>
  );
}
