import React, { useState, useCallback, useEffect, useRef } from "react";
import { X, ChevronDown, Circle } from "lucide-react";
import { MessageTimeline } from "./MessageTimeline";
import { PromptInput } from "./PromptInput";
import { useResizable } from "./hooks/useResizable";
import { fetchSessionMessages, sendMessage, abortSession } from "./api";
import type { SessionInfo, AppState, ModelRef, ImageAttachment } from "./api";
import type { Message } from "./types";

interface SplitViewProps {
  primarySessionId: string | null;
  secondarySessionId: string | null;
  onChangeSecondary: (sessionId: string | null) => void;
  onClose: () => void;
  sessions: SessionInfo[];
  appState: AppState | null;
  selectedModel?: string;
  projectIndex: number;
}

async function loadMessages(sessionId: string): Promise<Message[]> {
  try { return await fetchSessionMessages(sessionId); }
  catch { return []; }
}

/** Hook: fetch + poll messages for a session, polling every 3s while busy. */
function useSessionMessages(sessionId: string | null, isBusy: boolean) {
  const [messages, setMessages] = useState<Message[]>([]);
  useEffect(() => {
    if (!sessionId) { setMessages([]); return; }
    loadMessages(sessionId).then(setMessages);
  }, [sessionId]);
  useEffect(() => {
    if (!sessionId || !isBusy) return;
    const id = setInterval(() => loadMessages(sessionId).then(setMessages), 3000);
    return () => clearInterval(id);
  }, [sessionId, isBusy]);
  return [messages, setMessages] as const;
}

const StatusDot: React.FC<{ busy: boolean }> = ({ busy }) => (
  <Circle size={8} fill={busy ? "var(--color-warning)" : "var(--color-success)"} stroke="none" />
);

export function SplitView({
  primarySessionId, secondarySessionId, onChangeSecondary,
  onClose, sessions, appState, selectedModel, projectIndex,
}: SplitViewProps) {
  const { size, handleProps } = useResizable({
    initialSize: window.innerWidth / 2, minSize: 300, maxSize: window.innerWidth - 300,
  });

  const [primarySending, setPrimarySending] = useState(false);
  const [secondarySending, setSecondarySending] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  // Derive busy from appState
  const busySet = new Set(appState?.projects[projectIndex]?.busy_sessions ?? []);
  const primaryBusy = primarySessionId ? busySet.has(primarySessionId) : false;
  const secondaryBusy = secondarySessionId ? busySet.has(secondarySessionId) : false;

  const [primaryMessages, setPrimaryMessages] = useSessionMessages(primarySessionId, primaryBusy);
  const [secondaryMessages, setSecondaryMessages] = useSessionMessages(secondarySessionId, secondaryBusy);

  const modelRef: ModelRef | undefined = selectedModel
    ? { providerID: "", modelID: selectedModel } : undefined;

  const handleSendPrimary = useCallback(async (text: string, images?: ImageAttachment[]) => {
    if (!primarySessionId || primarySending) return;
    setPrimarySending(true);
    try {
      await sendMessage(primarySessionId, text, modelRef, images);
      setPrimaryMessages(await loadMessages(primarySessionId));
    } catch { /* ignore */ } finally { setPrimarySending(false); }
  }, [primarySessionId, primarySending, modelRef, setPrimaryMessages]);

  const handleSendSecondary = useCallback(async (text: string, images?: ImageAttachment[]) => {
    if (!secondarySessionId || secondarySending) return;
    setSecondarySending(true);
    const optId = `opt-${Date.now()}`;
    const optimistic: Message = {
      info: { role: "user", messageID: optId, sessionID: secondarySessionId },
      parts: [{ type: "text", text }],
    };
    setSecondaryMessages((prev) => [...prev, optimistic]);
    try {
      await sendMessage(secondarySessionId, text, modelRef, images);
      setSecondaryMessages(await loadMessages(secondarySessionId));
    } catch {
      setSecondaryMessages((prev) => prev.filter((m) => m.info.messageID !== optId));
    } finally { setSecondarySending(false); }
  }, [secondarySessionId, secondarySending, modelRef, setSecondaryMessages]);

  const handleAbortPrimary = useCallback(async () => {
    if (primarySessionId) await abortSession(primarySessionId).catch(() => {});
  }, [primarySessionId]);
  const handleAbortSecondary = useCallback(async () => {
    if (secondarySessionId) await abortSession(secondarySessionId).catch(() => {});
  }, [secondarySessionId]);

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const noopVoid = useCallback(() => {}, []);
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const noopCmd = useCallback((_c: string, _a?: string) => {}, []);
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const noopAgent = useCallback((_agent: string) => {}, []);

  const title = (id: string | null) => {
    if (!id) return "No session";
    return sessions.find((s) => s.id === id)?.title || id.slice(0, 8);
  };

  const filteredSessions = sessions.filter(
    (s) => s.id !== primarySessionId && s.id !== secondarySessionId,
  );

  // Close picker on outside click
  const pickerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!pickerOpen) return;
    const h = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) setPickerOpen(false);
    };
    document.addEventListener("mousedown", h);
    return () => document.removeEventListener("mousedown", h);
  }, [pickerOpen]);

  const inputProps = (sid: string | null, busy: boolean, sending: boolean,
    onSend: (t: string, i?: ImageAttachment[]) => void, onAbort: () => void) => ({
    onSend, onAbort, onCommand: noopCmd, onOpenModelPicker: noopVoid,
    isBusy: busy, isSending: sending, disabled: !sid, sessionId: sid,
    currentModel: selectedModel ?? null, currentAgent: "coder" as const, onAgentChange: noopAgent,
  });

  return (
    <div className="split-view" style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left pane */}
      <div className="split-view-pane split-view-left"
        style={{ width: size, flexShrink: 0, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div className="split-view-pane-header">
          <span className="split-view-pane-title">
            <StatusDot busy={primaryBusy} /> {title(primarySessionId)}
          </span>
        </div>
        <MessageTimeline messages={primaryMessages} sessionStatus={primaryBusy ? "busy" : "idle"}
          activeSessionId={primarySessionId} appState={appState} />
        <PromptInput {...inputProps(primarySessionId, primaryBusy, primarySending, handleSendPrimary, handleAbortPrimary)} />
      </div>

      <div {...handleProps} />

      {/* Right pane */}
      <div className="split-view-pane split-view-right"
        style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div className="split-view-pane-header">
          <span className="split-view-pane-title" style={{ position: "relative" }} ref={pickerRef}>
            <StatusDot busy={secondaryBusy} />
            <button className="split-view-picker-btn" onClick={() => setPickerOpen((p) => !p)}
              style={{ background: "none", border: "none", color: "inherit", cursor: "pointer",
                display: "inline-flex", alignItems: "center", gap: 4 }}>
              {title(secondarySessionId)} <ChevronDown size={14} />
            </button>
            {pickerOpen && (
              <div className="split-view-picker" style={{ position: "absolute", top: "100%", left: 0, zIndex: 100 }}>
                {filteredSessions.length === 0 && (
                  <div className="split-view-picker-empty" style={{ padding: "8px 12px", opacity: 0.6 }}>No other sessions</div>
                )}
                {filteredSessions.map((s) => (
                  <button key={s.id} className="split-view-picker-item"
                    onClick={() => { onChangeSecondary(s.id); setPickerOpen(false); }}>
                    {s.title || s.id.slice(0, 8)}
                  </button>
                ))}
              </div>
            )}
          </span>
          <button className="split-view-close-btn" onClick={onClose} title="Close split view"
            style={{ background: "none", border: "none", color: "inherit", cursor: "pointer", marginLeft: "auto" }}>
            <X size={16} />
          </button>
        </div>

        {secondarySessionId ? (
          <>
            <MessageTimeline messages={secondaryMessages} sessionStatus={secondaryBusy ? "busy" : "idle"}
              activeSessionId={secondarySessionId} appState={appState} />
            <PromptInput {...inputProps(secondarySessionId, secondaryBusy, secondarySending, handleSendSecondary, handleAbortSecondary)} />
          </>
        ) : (
          <div className="split-view-picker"
            style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 8 }}>
            <p style={{ opacity: 0.6 }}>Select a session to compare</p>
            {filteredSessions.map((s) => (
              <button key={s.id} className="split-view-picker-item" onClick={() => onChangeSecondary(s.id)}>
                {s.title || s.id.slice(0, 8)}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
