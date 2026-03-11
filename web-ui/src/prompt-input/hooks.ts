import { useState, useRef, useCallback, useEffect, type ClipboardEvent, type DragEvent } from "react";
import { fetchAgents, type AgentInfo, type ImageAttachment } from "../api";
import {
  ACCEPTED_IMAGE_TYPES, MAX_IMAGE_SIZE, DEFAULT_AGENTS,
  selectableAgents, fileToImageAttachment,
} from "./helpers";

// ── useAgents ───────────────────────────────────────────────────

export function useAgents(currentAgent: string, onAgentChange: (agent: string) => void) {
  const [allAgents, setAllAgents] = useState<AgentInfo[]>(DEFAULT_AGENTS);

  useEffect(() => {
    fetchAgents().then((fetched) => {
      if (fetched.length > 0) {
        setAllAgents(fetched);
        const selectable = selectableAgents(fetched);
        if (selectable.length > 0 && (!currentAgent || !selectable.some((a) => a.id === currentAgent))) {
          onAgentChange(selectable[0].id);
        }
      }
    });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const agents = selectableAgents(allAgents);
  const mentionableAgents = allAgents.filter((a) => !a.hidden && a.mode !== "primary");
  return { allAgents, agents, mentionableAgents };
}

// ── useAttachments ──────────────────────────────────────────────

export function useAttachments() {
  const [attachments, setAttachments] = useState<ImageAttachment[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const dragCountRef = useRef(0);

  const addImageFiles = useCallback(async (files: FileList | File[]) => {
    const imageFiles = Array.from(files).filter(
      (f) => ACCEPTED_IMAGE_TYPES.has(f.type) && f.size <= MAX_IMAGE_SIZE,
    );
    if (imageFiles.length === 0) return;
    try {
      const newAttachments = await Promise.all(imageFiles.map(fileToImageAttachment));
      setAttachments((prev) => [...prev, ...newAttachments]);
    } catch { /* silently ignore read errors */ }
  }, []);

  const removeAttachment = useCallback((index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const clearAttachments = useCallback(() => setAttachments([]), []);

  const handlePaste = useCallback((e: ClipboardEvent<HTMLTextAreaElement>) => {
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
    if (imageFiles.length > 0) { e.preventDefault(); addImageFiles(imageFiles); }
  }, [addImageFiles]);

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault(); e.stopPropagation();
    dragCountRef.current++;
    if (e.dataTransfer?.types?.includes("Files")) setDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault(); e.stopPropagation();
    dragCountRef.current--;
    if (dragCountRef.current <= 0) { dragCountRef.current = 0; setDragOver(false); }
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => { e.preventDefault(); e.stopPropagation(); }, []);

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault(); e.stopPropagation();
    dragCountRef.current = 0; setDragOver(false);
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) addImageFiles(files);
  }, [addImageFiles]);

  const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) addImageFiles(files);
    e.target.value = "";
  }, [addImageFiles]);

  return {
    attachments, dragOver, fileInputRef,
    addImageFiles, removeAttachment, clearAttachments,
    handlePaste, handleDragEnter, handleDragLeave, handleDragOver, handleDrop, handleFileSelect,
  };
}

// ── useAtMention ────────────────────────────────────────────────

export function useAtMention(
  allAgents: AgentInfo[],
  mentionableAgents: AgentInfo[],
  textareaRef: React.RefObject<HTMLTextAreaElement | null>,
  text: string,
  setText: (v: string) => void,
) {
  const [agentMentions, setAgentMentions] = useState<string[]>([]);
  const [showAtPopover, setShowAtPopover] = useState(false);
  const [atFilter, setAtFilter] = useState("");
  const atPopoverRef = useRef<HTMLDivElement>(null);

  // Close on outside click
  useEffect(() => {
    if (!showAtPopover) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (atPopoverRef.current && !atPopoverRef.current.contains(e.target as Node)) {
        setShowAtPopover(false); setAtFilter("");
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showAtPopover]);

  /** Update @mention state from input change */
  const updateAtState = useCallback((val: string, cursorPos: number) => {
    const before = val.slice(0, cursorPos);
    const atMatch = before.match(/@(\w*)$/);
    if (atMatch && mentionableAgents.length > 0) {
      setShowAtPopover(true); setAtFilter(atMatch[1].toLowerCase());
    } else {
      setShowAtPopover(false); setAtFilter("");
    }
  }, [mentionableAgents]);

  const handleAtAgentSelect = useCallback((agentId: string) => {
    setShowAtPopover(false); setAtFilter("");
    const el = textareaRef.current;
    if (!el) return;
    const pos = el.selectionStart ?? text.length;
    const before = text.slice(0, pos);
    const after = text.slice(pos);
    const atIdx = before.lastIndexOf("@");
    if (atIdx === -1) return;
    setText(before.slice(0, atIdx) + after);
    if (!agentMentions.includes(agentId)) setAgentMentions((prev) => [...prev, agentId]);
    setTimeout(() => {
      const t = textareaRef.current;
      if (t) { t.focus(); t.setSelectionRange(atIdx, atIdx); }
    }, 0);
  }, [text, setText, agentMentions, textareaRef]);

  const clearMentions = useCallback(() => setAgentMentions([]), []);

  const filteredMentionAgents = mentionableAgents.filter(
    (a) => atFilter === "" || a.id.toLowerCase().includes(atFilter) || a.label.toLowerCase().includes(atFilter),
  );

  return {
    agentMentions, showAtPopover, atPopoverRef,
    filteredMentionAgents, setAgentMentions,
    updateAtState, handleAtAgentSelect, clearMentions,
  };
}
