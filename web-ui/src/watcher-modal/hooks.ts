import { useState, useEffect, useCallback } from "react";
import {
  getWatcherSessions,
  getWatcher,
  createWatcher,
  deleteWatcher,
  getWatcherMessages,
} from "../api";
import type {
  WatcherSessionEntry,
  WatcherConfigResponse,
  WatcherMessageEntry,
} from "../api";

const DEFAULT_CONTINUATION_MSG = "Continue.";
const DEFAULT_IDLE_TIMEOUT = 10;
const DEFAULT_HANG_MSG =
  "The previous attempt appears to have stalled. Please retry the task.";
const DEFAULT_HANG_TIMEOUT = 180;

export function useWatcherState() {
  const [sessions, setSessions] = useState<WatcherSessionEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Config form state
  const [continuationMsg, setContinuationMsg] = useState(DEFAULT_CONTINUATION_MSG);
  const [idleTimeout, setIdleTimeout] = useState(DEFAULT_IDLE_TIMEOUT);
  const [includeOriginal, setIncludeOriginal] = useState(false);
  const [originalMessage, setOriginalMessage] = useState<string | null>(null);
  const [hangMessage, setHangMessage] = useState(DEFAULT_HANG_MSG);
  const [hangTimeout, setHangTimeout] = useState(DEFAULT_HANG_TIMEOUT);
  const [existingWatcher, setExistingWatcher] =
    useState<WatcherConfigResponse | null>(null);

  // Original message picker
  const [showMsgPicker, setShowMsgPicker] = useState(false);
  const [userMessages, setUserMessages] = useState<WatcherMessageEntry[]>([]);
  const [loadingMsgs, setLoadingMsgs] = useState(false);

  // Load sessions
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getWatcherSessions()
      .then((data) => {
        if (cancelled) return;
        setSessions(data);
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
    return () => {
      cancelled = true;
    };
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
        setExistingWatcher(null);
        setContinuationMsg(DEFAULT_CONTINUATION_MSG);
        setIdleTimeout(DEFAULT_IDLE_TIMEOUT);
        setIncludeOriginal(false);
        setOriginalMessage(null);
        setHangMessage(DEFAULT_HANG_MSG);
        setHangTimeout(DEFAULT_HANG_TIMEOUT);
      });
    return () => {
      cancelled = true;
    };
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
      const updated = await getWatcherSessions();
      setSessions(updated);
    } catch {
      setError("Failed to delete watcher");
    } finally {
      setSaving(false);
    }
  }, [selectedId]);

  return {
    sessions,
    selectedId,
    setSelectedId,
    loading,
    saving,
    error,
    continuationMsg,
    setContinuationMsg,
    idleTimeout,
    setIdleTimeout,
    includeOriginal,
    setIncludeOriginal,
    originalMessage,
    setOriginalMessage,
    hangMessage,
    setHangMessage,
    hangTimeout,
    setHangTimeout,
    existingWatcher,
    showMsgPicker,
    setShowMsgPicker,
    userMessages,
    loadingMsgs,
    loadMessages,
    handleSave,
    handleDelete,
  };
}
