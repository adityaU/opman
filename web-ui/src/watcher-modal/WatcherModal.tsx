import React, { useRef } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Eye, X } from "lucide-react";
import type { WatcherModalProps } from "./types";
import { useWatcherState } from "./hooks";
import { SessionList } from "./SessionList";
import { ConfigForm } from "./ConfigForm";

export function WatcherModal({ onClose, activeSessionId }: WatcherModalProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const formRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const state = useWatcherState();

  const statusLabel = state.existingWatcher?.status || "inactive";
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
          {state.existingWatcher && (
            <span
              className="watcher-status-badge"
              style={{ color: statusColor }}
            >
              {statusLabel === "idle_countdown"
                ? `idle ${state.existingWatcher.idle_since_secs ?? 0}s / ${state.idleTimeout}s`
                : statusLabel}
            </span>
          )}
          <button
            className="watcher-modal-close"
            onClick={onClose}
            aria-label="Close watcher"
          >
            <X size={14} />
          </button>
        </div>

        <div className="watcher-modal-body">
          <SessionList
            sessions={state.sessions}
            selectedId={state.selectedId}
            loading={state.loading}
            onSelect={state.setSelectedId}
          />

          <ConfigForm
            selectedId={state.selectedId}
            continuationMsg={state.continuationMsg}
            setContinuationMsg={state.setContinuationMsg}
            idleTimeout={state.idleTimeout}
            setIdleTimeout={state.setIdleTimeout}
            includeOriginal={state.includeOriginal}
            setIncludeOriginal={state.setIncludeOriginal}
            originalMessage={state.originalMessage}
            setOriginalMessage={state.setOriginalMessage}
            hangMessage={state.hangMessage}
            setHangMessage={state.setHangMessage}
            hangTimeout={state.hangTimeout}
            setHangTimeout={state.setHangTimeout}
            existingWatcher={state.existingWatcher}
            error={state.error}
            saving={state.saving}
            showMsgPicker={state.showMsgPicker}
            setShowMsgPicker={state.setShowMsgPicker}
            userMessages={state.userMessages}
            loadingMsgs={state.loadingMsgs}
            loadMessages={state.loadMessages}
            handleSave={state.handleSave}
            handleDelete={state.handleDelete}
            formRef={formRef}
          />
        </div>

        {/* Footer */}
        <div className="watcher-modal-footer">
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
