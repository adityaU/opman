import React, { useState, useEffect, useCallback, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import {
  getWatcherSessions,
  getWatcher,
  createWatcher,
  deleteWatcher,
  getWatcherMessages,
} from "./api";
import type {
  WatcherSessionEntry,
  WatcherConfigResponse,
  WatcherMessageEntry,
} from "./api";
import {
  Eye,
  EyeOff,
  X,
  Loader2,
  Play,
  Trash2,
  ChevronDown,
  Clock,
  AlertTriangle,
} from "lucide-react";

interface Props {
  onClose: () => void;
  activeSessionId: string | null;
}

type SessionGroup = "current" | "watched" | "active" | "other";

function groupSessions(
  sessions: WatcherSessionEntry[]
): Record<SessionGroup, WatcherSessionEntry[]> {
  const groups: Record<SessionGroup, WatcherSessionEntry[]> = {
    current: [],
    watched: [],
    active: [],
    other: [],
  };
  for (const s of sessions) {
    if (s.is_current) groups.current.push(s);
    else if (s.has_watcher) groups.watched.push(s);
    else if (s.is_active) groups.active.push(s);
    else groups.other.push(s);
  }
  return groups;
}

const GROUP_LABELS: Record<SessionGroup, string> = {
  current: "Current Session",
  watched: "Watched",
  active: "Active Sessions",
  other: "Other Sessions",
};

export function WatcherModal({ onClose, activeSessionId }: Props) {
  const [sessions, setSessions] = useState<WatcherSessionEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Config form state
  const [continuationMsg, setContinuationMsg] = useState("Continue.");
  const [idleTimeout, setIdleTimeout] = useState(10);
  const [includeOriginal, setIncludeOriginal] = useState(false);
  const [originalMessage, setOriginalMessage] = useState<string | null>(null);
  const [hangMessage, setHangMessage] = useState(
    "The previous attempt appears to have stalled. Please retry the task."
  );
  const [hangTimeout, setHangTimeout] = useState(180);
  const [existingWatcher, setExistingWatcher] =
    useState<WatcherConfigResponse | null>(null);

  // Original message picker
  const [showMsgPicker, setShowMsgPicker] = useState(false);
  const [userMessages, setUserMessages] = useState<WatcherMessageEntry[]>([]);
  const [loadingMsgs, setLoadingMsgs] = useState(false);

  const formRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  // Load sessions
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getWatcherSessions()
      .then((data) => {
        if (cancelled) return;
        setSessions(data);
        // Auto-select current session or first watched session
        const current = data.find((s) => s.is_current);
        const watched = data.find((s) => s.has_watcher);
        const fallback = current || watched || (data.length > 0 ? data[0] : null);
        if (fallback) setSelectedId(fallback.session_id);
        setLoading(false);
      })
      .catch(() => {
        if (!cancelled) {
          setError("Failed to load sessions");
          setLoading(false);
        }
      });
    return () => { cancelled = true; };
  }, []);

  // Load watcher config when selection changes
  useEffect(() => {
    if (!selectedId) return;
    let cancelled = false;
    setExistingWatcher(null);
    getWatcher(selectedId)
      .then((config) => {
        if (cancelled) return;
        setExistingWatcher(config);
        setContinuationMsg(config.continuation_message);
        setIdleTimeout(config.idle_timeout_secs);
        setIncludeOriginal(config.include_original);
        setOriginalMessage(config.original_message);
        setHangMessage(config.hang_message);
        setHangTimeout(config.hang_timeout_secs);
      })
      .catch(() => {
        if (cancelled) return;
        // No watcher exists — reset to defaults
        setExistingWatcher(null);
        setContinuationMsg("Continue.");
        setIdleTimeout(10);
        setIncludeOriginal(false);
        setOriginalMessage(null);
        setHangMessage(
          "The previous attempt appears to have stalled. Please retry the task."
        );
        setHangTimeout(180);
      });
    return () => { cancelled = true; };
  }, [selectedId]);

  // Load user messages for original-message picker
  const loadMessages = useCallback(async () => {
    if (!selectedId) return;
    setLoadingMsgs(true);
    try {
      const msgs = await getWatcherMessages(selectedId);
      setUserMessages(msgs);
      setShowMsgPicker(true);
    } catch {
      setUserMessages([]);
      setShowMsgPicker(true);
    } finally {
      setLoadingMsgs(false);
    }
  }, [selectedId]);

  const handleSave = useCallback(async () => {
    if (!selectedId) return;
    const session = sessions.find((s) => s.session_id === selectedId);
    if (!session) return;
    setSaving(true);
    setError(null);
    try {
      const resp = await createWatcher({
        session_id: selectedId,
        project_idx: session.project_idx,
        idle_timeout_secs: idleTimeout,
        continuation_message: continuationMsg,
        include_original: includeOriginal,
        original_message: originalMessage,
        hang_message: hangMessage,
        hang_timeout_secs: hangTimeout,
      });
      setExistingWatcher(resp);
      // Refresh sessions to update has_watcher
      const updated = await getWatcherSessions();
      setSessions(updated);
    } catch {
      setError("Failed to save watcher");
    } finally {
      setSaving(false);
    }
  }, [
    selectedId,
    sessions,
    idleTimeout,
    continuationMsg,
    includeOriginal,
    originalMessage,
    hangMessage,
    hangTimeout,
  ]);

  const handleDelete = useCallback(async () => {
    if (!selectedId) return;
    setSaving(true);
    setError(null);
    try {
      await deleteWatcher(selectedId);
      setExistingWatcher(null);
      // Refresh
      const updated = await getWatcherSessions();
      setSessions(updated);
    } catch {
      setError("Failed to delete watcher");
    } finally {
      setSaving(false);
    }
  }, [selectedId]);

  const selectedSession = sessions.find((s) => s.session_id === selectedId);
  const grouped = groupSessions(sessions);

  const statusLabel = existingWatcher?.status || "inactive";
  const statusColor =
    statusLabel === "idle_countdown"
      ? "var(--color-success)"
      : statusLabel === "running"
        ? "var(--color-warning)"
        : statusLabel === "waiting"
          ? "var(--color-text-muted)"
          : "var(--color-text-muted)";

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="watcher-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Session watcher"
        ref={modalRef}
      >
        {/* Header */}
        <div className="watcher-modal-header">
          <Eye size={14} />
          <span>Session Watcher</span>
          {existingWatcher && (
            <span className="watcher-status-badge" style={{ color: statusColor }}>
              {statusLabel === "idle_countdown"
                ? `idle ${existingWatcher.idle_since_secs ?? 0}s / ${idleTimeout}s`
                : statusLabel}
            </span>
          )}
          <button className="watcher-modal-close" onClick={onClose} aria-label="Close watcher">
            <X size={14} />
          </button>
        </div>

        <div className="watcher-modal-body">
          {/* Left panel: session list */}
          <div className="watcher-session-list">
            {loading ? (
              <div className="watcher-empty">
                <Loader2 size={16} className="spinning" />
                <span>Loading sessions...</span>
              </div>
            ) : sessions.length === 0 ? (
              <div className="watcher-empty">No sessions available</div>
            ) : (
              (Object.keys(grouped) as SessionGroup[]).map((group) => {
                const items = grouped[group];
                if (items.length === 0) return null;
                return (
                  <div key={group} className="watcher-session-group">
                    <div className="watcher-group-label">{GROUP_LABELS[group]}</div>
                    {items.map((s) => (
                      <button
                        key={s.session_id}
                        className={`watcher-session-item ${s.session_id === selectedId ? "selected" : ""}`}
                        onClick={() => setSelectedId(s.session_id)}
                      >
                        <span className="watcher-session-title">
                          {s.title || s.session_id.slice(0, 12)}
                        </span>
                        {s.has_watcher && (
                          <Eye size={11} className="watcher-session-icon" />
                        )}
                        <span className="watcher-session-project">
                          {s.project_name}
                        </span>
                      </button>
                    ))}
                  </div>
                );
              })
            )}
          </div>

          {/* Right panel: config form */}
          <div className="watcher-config-form" ref={formRef}>
            {!selectedId ? (
              <div className="watcher-empty">Select a session to configure</div>
            ) : (
              <>
                <div className="watcher-form-section">
                  <label className="watcher-form-label">
                    Continuation Message
                  </label>
                  <textarea
                    className="watcher-form-textarea"
                    value={continuationMsg}
                    onChange={(e) => setContinuationMsg(e.target.value)}
                    rows={3}
                    placeholder="Message sent when session goes idle..."
                  />
                </div>

                <div className="watcher-form-section">
                  <label className="watcher-form-label">
                    <Clock size={12} />
                    Idle Timeout (seconds)
                  </label>
                  <input
                    type="number"
                    className="watcher-form-input"
                    value={idleTimeout}
                    onChange={(e) =>
                      setIdleTimeout(Math.max(1, parseInt(e.target.value) || 1))
                    }
                    min={1}
                  />
                  <span className="watcher-form-hint">
                    Seconds to wait after session goes idle before sending continuation
                  </span>
                </div>

                <div className="watcher-form-section">
                  <label className="watcher-form-label watcher-form-check">
                    <input
                      type="checkbox"
                      checked={includeOriginal}
                      onChange={(e) => setIncludeOriginal(e.target.checked)}
                    />
                    Include original message
                  </label>
                  {includeOriginal && (
                    <div className="watcher-original-msg">
                      {originalMessage ? (
                        <div className="watcher-original-preview">
                          <span className="watcher-original-text">
                            {originalMessage.length > 120
                              ? originalMessage.slice(0, 120) + "..."
                              : originalMessage}
                          </span>
                          <button
                            className="watcher-btn-sm"
                            onClick={() => {
                              setOriginalMessage(null);
                            }}
                          >
                            Clear
                          </button>
                        </div>
                      ) : (
                        <button
                          className="watcher-btn-sm"
                          onClick={loadMessages}
                          disabled={loadingMsgs}
                        >
                          {loadingMsgs ? (
                            <Loader2 size={11} className="spinning" />
                          ) : (
                            <ChevronDown size={11} />
                          )}
                          Pick from session messages
                        </button>
                      )}
                      {showMsgPicker && (
                        <div className="watcher-msg-picker">
                          {userMessages.length === 0 ? (
                            <div className="watcher-msg-picker-empty">
                              No user messages found
                            </div>
                          ) : (
                            userMessages.slice(0, 10).map((m, i) => (
                              <button
                                key={i}
                                className="watcher-msg-picker-item"
                                onClick={() => {
                                  setOriginalMessage(m.text);
                                  setShowMsgPicker(false);
                                }}
                              >
                                {m.text.length > 100
                                  ? m.text.slice(0, 100) + "..."
                                  : m.text}
                              </button>
                            ))
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </div>

                <div className="watcher-form-divider" />

                <div className="watcher-form-section">
                  <label className="watcher-form-label">
                    <AlertTriangle size={12} />
                    Hang Detection Message
                  </label>
                  <textarea
                    className="watcher-form-textarea"
                    value={hangMessage}
                    onChange={(e) => setHangMessage(e.target.value)}
                    rows={2}
                    placeholder="Message sent when session appears hung..."
                  />
                </div>

                <div className="watcher-form-section">
                  <label className="watcher-form-label">
                    <Clock size={12} />
                    Hang Timeout (seconds)
                  </label>
                  <input
                    type="number"
                    className="watcher-form-input"
                    value={hangTimeout}
                    onChange={(e) =>
                      setHangTimeout(Math.max(10, parseInt(e.target.value) || 10))
                    }
                    min={10}
                  />
                  <span className="watcher-form-hint">
                    Seconds of silence before session is considered hung
                  </span>
                </div>

                {error && <div className="watcher-form-error">{error}</div>}

                {/* Actions */}
                <div className="watcher-form-actions">
                  {existingWatcher ? (
                    <>
                      <button
                        className="watcher-btn watcher-btn-primary"
                        onClick={handleSave}
                        disabled={saving || !continuationMsg.trim()}
                      >
                        {saving ? (
                          <Loader2 size={13} className="spinning" />
                        ) : (
                          <Play size={13} />
                        )}
                        Update Watcher
                      </button>
                      <button
                        className="watcher-btn watcher-btn-danger"
                        onClick={handleDelete}
                        disabled={saving}
                      >
                        <Trash2 size={13} />
                        Remove
                      </button>
                    </>
                  ) : (
                    <button
                      className="watcher-btn watcher-btn-primary"
                      onClick={handleSave}
                      disabled={saving || !continuationMsg.trim()}
                    >
                      {saving ? (
                        <Loader2 size={13} className="spinning" />
                      ) : (
                        <Eye size={13} />
                      )}
                      Start Watcher
                    </button>
                  )}
                </div>
              </>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="watcher-modal-footer">
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
