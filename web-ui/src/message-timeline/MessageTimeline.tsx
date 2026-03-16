import React, { useRef, useEffect, useCallback, useMemo, useState, useLayoutEffect } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown } from "lucide-react";
import type { SessionInfo } from "../api";
import { MessageTurn } from "../MessageTurn";
import {
  MessageTimelineProps,
  MessageGroup,
  VIRTUALIZE_THRESHOLD,
  SCROLL_DIRECTION_THRESHOLD,
  groupMessages,
} from "./types";
import { MessageShimmer, WelcomeEmpty, NewSessionEmpty } from "./components";

export function MessageTimeline({
  messages,
  sessionStatus,
  activeSessionId,
  isLoadingMessages = false,
  isLoadingOlder = false,
  hasOlderMessages = false,
  totalMessageCount = 0,
  onLoadOlder,
  appState,
  defaultModel,
  onSendPrompt,
  subagentMessages,
  searchMatchIds,
  activeSearchMatchId,
  isBookmarked,
  onToggleBookmark,
  onScrollDirection,
  onOpenSession,
}: MessageTimelineProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const shouldAutoScrollRef = useRef(true);
  const [showJumpToBottom, setShowJumpToBottom] = useState(false);
  const loadOlderLockRef = useRef(false);

  // Scroll direction detection refs
  const lastScrollTopRef = useRef(0);
  const cumulativeDeltaRef = useRef(0);
  const directionRafRef = useRef(0);

  // Derive session directory for empty state
  const sessionDirectory = useMemo(() => {
    if (!appState || !activeSessionId) return null;
    const project = appState.projects[appState.active_project];
    if (!project) return null;
    const session = project.sessions.find((s) => s.id === activeSessionId);
    if (session?.directory) return session.directory;
    return project.path || null;
  }, [appState, activeSessionId]);

  const prevGroupsRef = useRef<MessageGroup[]>([]);
  const groups = useMemo(() => {
    const next = groupMessages(messages, prevGroupsRef.current);
    prevGroupsRef.current = next;
    return next;
  }, [messages]);

  // Find child sessions for task tool → child session matching
  const childSessions = useMemo(() => {
    if (!appState || !activeSessionId) return [];
    const children: SessionInfo[] = [];
    for (const project of appState.projects) {
      for (const session of project.sessions) {
        if (session.parentID === activeSessionId) children.push(session);
      }
    }
    children.sort((a, b) => a.time.created - b.time.created);
    return children;
  }, [appState, activeSessionId]);

  const itemCount = groups.length;
  const useVirtual = groups.length >= VIRTUALIZE_THRESHOLD;

  const virtualizer = useVirtualizer({
    count: itemCount,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 160,
    overscan: 5,
  });

  // ── Scroll handling ──
  const programmaticScrollRef = useRef(false);

  const handleScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;
    const distFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight;
    const nearBottom = distFromBottom < 100;

    // Always track scroll position so direction detection stays accurate
    const currentScrollTop = container.scrollTop;
    const delta = currentScrollTop - lastScrollTopRef.current;
    lastScrollTopRef.current = currentScrollTop;

    // Programmatic scrolls (auto-scroll) — update UI state but skip
    // user-intent logic (direction detection, load-older, auto-scroll toggle)
    if (programmaticScrollRef.current) {
      programmaticScrollRef.current = false;
      cumulativeDeltaRef.current = 0; // reset so next real scroll is clean
      setShowJumpToBottom(!nearBottom);
      return;
    }

    if (delta < -5) shouldAutoScrollRef.current = false;
    if (nearBottom) shouldAutoScrollRef.current = true;
    setShowJumpToBottom(!shouldAutoScrollRef.current && !nearBottom);

    // Load older messages when near top
    if (onLoadOlder && hasOlderMessages && !loadOlderLockRef.current && container.scrollTop < 200) {
      loadOlderLockRef.current = true;
      const prevScrollHeight = container.scrollHeight;
      onLoadOlder().finally(() => {
        requestAnimationFrame(() => {
          const heightDiff = container.scrollHeight - prevScrollHeight;
          if (heightDiff > 0) container.scrollTop += heightDiff;
          loadOlderLockRef.current = false;
        });
      });
    }

    // Scroll direction detection (mobile dock collapse/expand)
    if (onScrollDirection) {
      if ((cumulativeDeltaRef.current > 0 && delta < 0) ||
          (cumulativeDeltaRef.current < 0 && delta > 0)) {
        cumulativeDeltaRef.current = 0;
      }
      cumulativeDeltaRef.current += delta;
      if (Math.abs(cumulativeDeltaRef.current) >= SCROLL_DIRECTION_THRESHOLD) {
        const direction = cumulativeDeltaRef.current < 0 ? "up" : "down";
        cumulativeDeltaRef.current = 0;
        if (directionRafRef.current) cancelAnimationFrame(directionRafRef.current);
        directionRafRef.current = requestAnimationFrame(() => {
          onScrollDirection(direction);
          directionRafRef.current = 0;
        });
      }
    }
  }, [onScrollDirection, onLoadOlder, hasOlderMessages]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    lastScrollTopRef.current = container.scrollTop;
    cumulativeDeltaRef.current = 0;
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => container.removeEventListener("scroll", handleScroll);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [handleScroll, messages.length]);

  // Content fingerprint for streaming auto-scroll
  const lastMsg = messages[messages.length - 1];
  const contentFingerprint = lastMsg ? (lastMsg.parts?.length ?? 0) : 0;

  useEffect(() => {
    if (!shouldAutoScrollRef.current) return;
    programmaticScrollRef.current = true;
    if (useVirtual) {
      virtualizer.scrollToIndex(itemCount - 1, { align: "end" });
    } else {
      const container = containerRef.current;
      if (container) container.scrollTop = container.scrollHeight;
    }
  }, [messages.length, sessionStatus, useVirtual, itemCount, virtualizer, contentFingerprint]);

  const scrollToBottom = useCallback(() => {
    programmaticScrollRef.current = true;
    if (useVirtual) {
      virtualizer.scrollToIndex(itemCount - 1, { align: "end" });
    } else {
      const container = containerRef.current;
      if (container) container.scrollTop = container.scrollHeight;
    }
    shouldAutoScrollRef.current = true;
    setShowJumpToBottom(false);
  }, [useVirtual, virtualizer, itemCount]);

  // ── Search: message ID → group index/key maps ──
  const messageIdToGroupIndex = useMemo(() => {
    const map = new Map<string, number>();
    groups.forEach((group, idx) => {
      for (const msg of group.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (id) map.set(id, idx);
      }
    });
    return map;
  }, [groups]);

  const messageIdToGroupKey = useMemo(() => {
    const map = new Map<string, string>();
    for (const group of groups) {
      for (const msg of group.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (id) map.set(id, group.key);
      }
    }
    return map;
  }, [groups]);

  // Scroll to active search match
  useEffect(() => {
    if (!activeSearchMatchId) return;
    const container = containerRef.current;
    if (!container) return;
    const groupKey = messageIdToGroupKey.get(activeSearchMatchId);
    if (!groupKey) return;

    if (useVirtual) {
      const groupIdx = messageIdToGroupIndex.get(activeSearchMatchId);
      if (groupIdx !== undefined) {
        virtualizer.scrollToIndex(groupIdx, { align: "center" });
        requestAnimationFrame(() => {
          const el = container.querySelector(`[data-group-key="${CSS.escape(groupKey)}"]`);
          el?.scrollIntoView({ behavior: "smooth", block: "center" });
        });
      }
    } else {
      const el = container.querySelector(`[data-group-key="${CSS.escape(groupKey)}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
    shouldAutoScrollRef.current = false;
  }, [activeSearchMatchId, useVirtual, virtualizer, messageIdToGroupIndex, messageIdToGroupKey]);

  // Find the last assistant message that hasn't completed yet.
  // User messages sent after this one are considered "queued".
  const pendingAssistantId = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      const m = messages[i];
      if (m.info.role !== "assistant") continue;
      const t = m.metadata?.time ?? (typeof m.info.time === "object" ? m.info.time : undefined);
      if (!t?.completed) return m.info.messageID || m.info.id || "";
    }
    return null;
  }, [messages]);

  // ── Stable turn props (must be before early returns to obey Rules of Hooks) ──
  const turnProps = useMemo(() => ({
    childSessions,
    onRetry: onSendPrompt,
    subagentMessages,
    searchMatchIds,
    activeSearchMatchId,
    isBookmarked,
    onToggleBookmark,
    sessionId: activeSessionId,
    onOpenSession,
    pendingAssistantId,
  }), [
    childSessions,
    onSendPrompt,
    subagentMessages,
    searchMatchIds,
    activeSearchMatchId,
    isBookmarked,
    onToggleBookmark,
    activeSessionId,
    onOpenSession,
    pendingAssistantId,
  ]);

  // ── Empty states ──
  if (!activeSessionId) return <WelcomeEmpty />;

  if (isLoadingMessages && messages.length === 0) {
    return (
      <div className="message-timeline" ref={containerRef}>
        <div className="message-timeline-inner"><MessageShimmer /></div>
      </div>
    );
  }

  if (messages.length === 0 && sessionStatus === "idle") {
    return (
      <NewSessionEmpty
        sessionDirectory={sessionDirectory}
        defaultModel={defaultModel}
        onSendPrompt={onSendPrompt}
      />
    );
  }

  const olderMessagesIndicator = isLoadingOlder ? (
    <div className="load-older-messages">
      <span className="load-older-spinner">Loading older messages...</span>
    </div>
  ) : null;

  const jumpButton = showJumpToBottom && (
    <button className="jump-to-bottom" onClick={scrollToBottom}>
      <ArrowDown size={14} />
      <span>Jump to bottom</span>
    </button>
  );

  if (useVirtual) {
    const virtualItems = virtualizer.getVirtualItems();
    return (
      <div className="message-timeline" ref={containerRef} role="log" aria-live="polite" aria-label="Chat messages">
        {olderMessagesIndicator}
        <div className="message-timeline-inner" style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}>
          {virtualItems.map((virtualRow) => {
            const group = groups[virtualRow.index];
            return (
              <div key={group.key} data-index={virtualRow.index} data-group-key={group.key} ref={virtualizer.measureElement}
                style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${virtualRow.start}px)` }}>
                <MessageTurn group={group} {...turnProps} />
              </div>
            );
          })}
        </div>
        {jumpButton}
      </div>
    );
  }

  return (
    <div className="message-timeline" ref={containerRef} role="log" aria-live="polite" aria-label="Chat messages">
      <div className="message-timeline-inner">
        {olderMessagesIndicator}
        {groups.map((group) => (
          <div key={group.key} data-group-key={group.key}>
            <MessageTurn group={group} {...turnProps} />
          </div>
        ))}
      </div>
      {jumpButton}
    </div>
  );
}
